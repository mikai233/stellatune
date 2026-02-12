// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'stellatune_core.dart';

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

/// @nodoc
mixin _$LibraryEvent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LibraryEvent()';
}


}

/// @nodoc
class $LibraryEventCopyWith<$Res>  {
$LibraryEventCopyWith(LibraryEvent _, $Res Function(LibraryEvent) __);
}


/// Adds pattern-matching-related methods to [LibraryEvent].
extension LibraryEventPatterns on LibraryEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( LibraryEvent_Roots value)?  roots,TResult Function( LibraryEvent_Folders value)?  folders,TResult Function( LibraryEvent_ExcludedFolders value)?  excludedFolders,TResult Function( LibraryEvent_Changed value)?  changed,TResult Function( LibraryEvent_Tracks value)?  tracks,TResult Function( LibraryEvent_ScanProgress value)?  scanProgress,TResult Function( LibraryEvent_ScanFinished value)?  scanFinished,TResult Function( LibraryEvent_SearchResult value)?  searchResult,TResult Function( LibraryEvent_Playlists value)?  playlists,TResult Function( LibraryEvent_PlaylistTracks value)?  playlistTracks,TResult Function( LibraryEvent_LikedTrackIds value)?  likedTrackIds,TResult Function( LibraryEvent_Error value)?  error,TResult Function( LibraryEvent_Log value)?  log,required TResult orElse(),}){
final _that = this;
switch (_that) {
case LibraryEvent_Roots() when roots != null:
return roots(_that);case LibraryEvent_Folders() when folders != null:
return folders(_that);case LibraryEvent_ExcludedFolders() when excludedFolders != null:
return excludedFolders(_that);case LibraryEvent_Changed() when changed != null:
return changed(_that);case LibraryEvent_Tracks() when tracks != null:
return tracks(_that);case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that);case LibraryEvent_SearchResult() when searchResult != null:
return searchResult(_that);case LibraryEvent_Playlists() when playlists != null:
return playlists(_that);case LibraryEvent_PlaylistTracks() when playlistTracks != null:
return playlistTracks(_that);case LibraryEvent_LikedTrackIds() when likedTrackIds != null:
return likedTrackIds(_that);case LibraryEvent_Error() when error != null:
return error(_that);case LibraryEvent_Log() when log != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( LibraryEvent_Roots value)  roots,required TResult Function( LibraryEvent_Folders value)  folders,required TResult Function( LibraryEvent_ExcludedFolders value)  excludedFolders,required TResult Function( LibraryEvent_Changed value)  changed,required TResult Function( LibraryEvent_Tracks value)  tracks,required TResult Function( LibraryEvent_ScanProgress value)  scanProgress,required TResult Function( LibraryEvent_ScanFinished value)  scanFinished,required TResult Function( LibraryEvent_SearchResult value)  searchResult,required TResult Function( LibraryEvent_Playlists value)  playlists,required TResult Function( LibraryEvent_PlaylistTracks value)  playlistTracks,required TResult Function( LibraryEvent_LikedTrackIds value)  likedTrackIds,required TResult Function( LibraryEvent_Error value)  error,required TResult Function( LibraryEvent_Log value)  log,}){
final _that = this;
switch (_that) {
case LibraryEvent_Roots():
return roots(_that);case LibraryEvent_Folders():
return folders(_that);case LibraryEvent_ExcludedFolders():
return excludedFolders(_that);case LibraryEvent_Changed():
return changed(_that);case LibraryEvent_Tracks():
return tracks(_that);case LibraryEvent_ScanProgress():
return scanProgress(_that);case LibraryEvent_ScanFinished():
return scanFinished(_that);case LibraryEvent_SearchResult():
return searchResult(_that);case LibraryEvent_Playlists():
return playlists(_that);case LibraryEvent_PlaylistTracks():
return playlistTracks(_that);case LibraryEvent_LikedTrackIds():
return likedTrackIds(_that);case LibraryEvent_Error():
return error(_that);case LibraryEvent_Log():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( LibraryEvent_Roots value)?  roots,TResult? Function( LibraryEvent_Folders value)?  folders,TResult? Function( LibraryEvent_ExcludedFolders value)?  excludedFolders,TResult? Function( LibraryEvent_Changed value)?  changed,TResult? Function( LibraryEvent_Tracks value)?  tracks,TResult? Function( LibraryEvent_ScanProgress value)?  scanProgress,TResult? Function( LibraryEvent_ScanFinished value)?  scanFinished,TResult? Function( LibraryEvent_SearchResult value)?  searchResult,TResult? Function( LibraryEvent_Playlists value)?  playlists,TResult? Function( LibraryEvent_PlaylistTracks value)?  playlistTracks,TResult? Function( LibraryEvent_LikedTrackIds value)?  likedTrackIds,TResult? Function( LibraryEvent_Error value)?  error,TResult? Function( LibraryEvent_Log value)?  log,}){
final _that = this;
switch (_that) {
case LibraryEvent_Roots() when roots != null:
return roots(_that);case LibraryEvent_Folders() when folders != null:
return folders(_that);case LibraryEvent_ExcludedFolders() when excludedFolders != null:
return excludedFolders(_that);case LibraryEvent_Changed() when changed != null:
return changed(_that);case LibraryEvent_Tracks() when tracks != null:
return tracks(_that);case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that);case LibraryEvent_SearchResult() when searchResult != null:
return searchResult(_that);case LibraryEvent_Playlists() when playlists != null:
return playlists(_that);case LibraryEvent_PlaylistTracks() when playlistTracks != null:
return playlistTracks(_that);case LibraryEvent_LikedTrackIds() when likedTrackIds != null:
return likedTrackIds(_that);case LibraryEvent_Error() when error != null:
return error(_that);case LibraryEvent_Log() when log != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( List<String> paths)?  roots,TResult Function( List<String> paths)?  folders,TResult Function( List<String> paths)?  excludedFolders,TResult Function()?  changed,TResult Function( String folder,  bool recursive,  String query,  List<TrackLite> items)?  tracks,TResult Function( PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanProgress,TResult Function( PlatformInt64 durationMs,  PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanFinished,TResult Function( String query,  List<TrackLite> items)?  searchResult,TResult Function( List<PlaylistLite> items)?  playlists,TResult Function( PlatformInt64 playlistId,  String query,  List<TrackLite> items)?  playlistTracks,TResult Function( Int64List trackIds)?  likedTrackIds,TResult Function( String message)?  error,TResult Function( String message)?  log,required TResult orElse(),}) {final _that = this;
switch (_that) {
case LibraryEvent_Roots() when roots != null:
return roots(_that.paths);case LibraryEvent_Folders() when folders != null:
return folders(_that.paths);case LibraryEvent_ExcludedFolders() when excludedFolders != null:
return excludedFolders(_that.paths);case LibraryEvent_Changed() when changed != null:
return changed();case LibraryEvent_Tracks() when tracks != null:
return tracks(_that.folder,_that.recursive,_that.query,_that.items);case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that.durationMs,_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_SearchResult() when searchResult != null:
return searchResult(_that.query,_that.items);case LibraryEvent_Playlists() when playlists != null:
return playlists(_that.items);case LibraryEvent_PlaylistTracks() when playlistTracks != null:
return playlistTracks(_that.playlistId,_that.query,_that.items);case LibraryEvent_LikedTrackIds() when likedTrackIds != null:
return likedTrackIds(_that.trackIds);case LibraryEvent_Error() when error != null:
return error(_that.message);case LibraryEvent_Log() when log != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( List<String> paths)  roots,required TResult Function( List<String> paths)  folders,required TResult Function( List<String> paths)  excludedFolders,required TResult Function()  changed,required TResult Function( String folder,  bool recursive,  String query,  List<TrackLite> items)  tracks,required TResult Function( PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)  scanProgress,required TResult Function( PlatformInt64 durationMs,  PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)  scanFinished,required TResult Function( String query,  List<TrackLite> items)  searchResult,required TResult Function( List<PlaylistLite> items)  playlists,required TResult Function( PlatformInt64 playlistId,  String query,  List<TrackLite> items)  playlistTracks,required TResult Function( Int64List trackIds)  likedTrackIds,required TResult Function( String message)  error,required TResult Function( String message)  log,}) {final _that = this;
switch (_that) {
case LibraryEvent_Roots():
return roots(_that.paths);case LibraryEvent_Folders():
return folders(_that.paths);case LibraryEvent_ExcludedFolders():
return excludedFolders(_that.paths);case LibraryEvent_Changed():
return changed();case LibraryEvent_Tracks():
return tracks(_that.folder,_that.recursive,_that.query,_that.items);case LibraryEvent_ScanProgress():
return scanProgress(_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_ScanFinished():
return scanFinished(_that.durationMs,_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_SearchResult():
return searchResult(_that.query,_that.items);case LibraryEvent_Playlists():
return playlists(_that.items);case LibraryEvent_PlaylistTracks():
return playlistTracks(_that.playlistId,_that.query,_that.items);case LibraryEvent_LikedTrackIds():
return likedTrackIds(_that.trackIds);case LibraryEvent_Error():
return error(_that.message);case LibraryEvent_Log():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( List<String> paths)?  roots,TResult? Function( List<String> paths)?  folders,TResult? Function( List<String> paths)?  excludedFolders,TResult? Function()?  changed,TResult? Function( String folder,  bool recursive,  String query,  List<TrackLite> items)?  tracks,TResult? Function( PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanProgress,TResult? Function( PlatformInt64 durationMs,  PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanFinished,TResult? Function( String query,  List<TrackLite> items)?  searchResult,TResult? Function( List<PlaylistLite> items)?  playlists,TResult? Function( PlatformInt64 playlistId,  String query,  List<TrackLite> items)?  playlistTracks,TResult? Function( Int64List trackIds)?  likedTrackIds,TResult? Function( String message)?  error,TResult? Function( String message)?  log,}) {final _that = this;
switch (_that) {
case LibraryEvent_Roots() when roots != null:
return roots(_that.paths);case LibraryEvent_Folders() when folders != null:
return folders(_that.paths);case LibraryEvent_ExcludedFolders() when excludedFolders != null:
return excludedFolders(_that.paths);case LibraryEvent_Changed() when changed != null:
return changed();case LibraryEvent_Tracks() when tracks != null:
return tracks(_that.folder,_that.recursive,_that.query,_that.items);case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that.durationMs,_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_SearchResult() when searchResult != null:
return searchResult(_that.query,_that.items);case LibraryEvent_Playlists() when playlists != null:
return playlists(_that.items);case LibraryEvent_PlaylistTracks() when playlistTracks != null:
return playlistTracks(_that.playlistId,_that.query,_that.items);case LibraryEvent_LikedTrackIds() when likedTrackIds != null:
return likedTrackIds(_that.trackIds);case LibraryEvent_Error() when error != null:
return error(_that.message);case LibraryEvent_Log() when log != null:
return log(_that.message);case _:
  return null;

}
}

}

/// @nodoc


class LibraryEvent_Roots extends LibraryEvent {
  const LibraryEvent_Roots({required final  List<String> paths}): _paths = paths,super._();
  

 final  List<String> _paths;
 List<String> get paths {
  if (_paths is EqualUnmodifiableListView) return _paths;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_paths);
}


/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_RootsCopyWith<LibraryEvent_Roots> get copyWith => _$LibraryEvent_RootsCopyWithImpl<LibraryEvent_Roots>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_Roots&&const DeepCollectionEquality().equals(other._paths, _paths));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_paths));

@override
String toString() {
  return 'LibraryEvent.roots(paths: $paths)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_RootsCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_RootsCopyWith(LibraryEvent_Roots value, $Res Function(LibraryEvent_Roots) _then) = _$LibraryEvent_RootsCopyWithImpl;
@useResult
$Res call({
 List<String> paths
});




}
/// @nodoc
class _$LibraryEvent_RootsCopyWithImpl<$Res>
    implements $LibraryEvent_RootsCopyWith<$Res> {
  _$LibraryEvent_RootsCopyWithImpl(this._self, this._then);

  final LibraryEvent_Roots _self;
  final $Res Function(LibraryEvent_Roots) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? paths = null,}) {
  return _then(LibraryEvent_Roots(
paths: null == paths ? _self._paths : paths // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class LibraryEvent_Folders extends LibraryEvent {
  const LibraryEvent_Folders({required final  List<String> paths}): _paths = paths,super._();
  

 final  List<String> _paths;
 List<String> get paths {
  if (_paths is EqualUnmodifiableListView) return _paths;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_paths);
}


/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_FoldersCopyWith<LibraryEvent_Folders> get copyWith => _$LibraryEvent_FoldersCopyWithImpl<LibraryEvent_Folders>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_Folders&&const DeepCollectionEquality().equals(other._paths, _paths));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_paths));

@override
String toString() {
  return 'LibraryEvent.folders(paths: $paths)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_FoldersCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_FoldersCopyWith(LibraryEvent_Folders value, $Res Function(LibraryEvent_Folders) _then) = _$LibraryEvent_FoldersCopyWithImpl;
@useResult
$Res call({
 List<String> paths
});




}
/// @nodoc
class _$LibraryEvent_FoldersCopyWithImpl<$Res>
    implements $LibraryEvent_FoldersCopyWith<$Res> {
  _$LibraryEvent_FoldersCopyWithImpl(this._self, this._then);

  final LibraryEvent_Folders _self;
  final $Res Function(LibraryEvent_Folders) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? paths = null,}) {
  return _then(LibraryEvent_Folders(
paths: null == paths ? _self._paths : paths // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class LibraryEvent_ExcludedFolders extends LibraryEvent {
  const LibraryEvent_ExcludedFolders({required final  List<String> paths}): _paths = paths,super._();
  

 final  List<String> _paths;
 List<String> get paths {
  if (_paths is EqualUnmodifiableListView) return _paths;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_paths);
}


/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_ExcludedFoldersCopyWith<LibraryEvent_ExcludedFolders> get copyWith => _$LibraryEvent_ExcludedFoldersCopyWithImpl<LibraryEvent_ExcludedFolders>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_ExcludedFolders&&const DeepCollectionEquality().equals(other._paths, _paths));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_paths));

@override
String toString() {
  return 'LibraryEvent.excludedFolders(paths: $paths)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_ExcludedFoldersCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_ExcludedFoldersCopyWith(LibraryEvent_ExcludedFolders value, $Res Function(LibraryEvent_ExcludedFolders) _then) = _$LibraryEvent_ExcludedFoldersCopyWithImpl;
@useResult
$Res call({
 List<String> paths
});




}
/// @nodoc
class _$LibraryEvent_ExcludedFoldersCopyWithImpl<$Res>
    implements $LibraryEvent_ExcludedFoldersCopyWith<$Res> {
  _$LibraryEvent_ExcludedFoldersCopyWithImpl(this._self, this._then);

  final LibraryEvent_ExcludedFolders _self;
  final $Res Function(LibraryEvent_ExcludedFolders) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? paths = null,}) {
  return _then(LibraryEvent_ExcludedFolders(
paths: null == paths ? _self._paths : paths // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class LibraryEvent_Changed extends LibraryEvent {
  const LibraryEvent_Changed(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_Changed);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LibraryEvent.changed()';
}


}




/// @nodoc


class LibraryEvent_Tracks extends LibraryEvent {
  const LibraryEvent_Tracks({required this.folder, required this.recursive, required this.query, required final  List<TrackLite> items}): _items = items,super._();
  

 final  String folder;
 final  bool recursive;
 final  String query;
 final  List<TrackLite> _items;
 List<TrackLite> get items {
  if (_items is EqualUnmodifiableListView) return _items;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_items);
}


/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_TracksCopyWith<LibraryEvent_Tracks> get copyWith => _$LibraryEvent_TracksCopyWithImpl<LibraryEvent_Tracks>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_Tracks&&(identical(other.folder, folder) || other.folder == folder)&&(identical(other.recursive, recursive) || other.recursive == recursive)&&(identical(other.query, query) || other.query == query)&&const DeepCollectionEquality().equals(other._items, _items));
}


@override
int get hashCode => Object.hash(runtimeType,folder,recursive,query,const DeepCollectionEquality().hash(_items));

@override
String toString() {
  return 'LibraryEvent.tracks(folder: $folder, recursive: $recursive, query: $query, items: $items)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_TracksCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_TracksCopyWith(LibraryEvent_Tracks value, $Res Function(LibraryEvent_Tracks) _then) = _$LibraryEvent_TracksCopyWithImpl;
@useResult
$Res call({
 String folder, bool recursive, String query, List<TrackLite> items
});




}
/// @nodoc
class _$LibraryEvent_TracksCopyWithImpl<$Res>
    implements $LibraryEvent_TracksCopyWith<$Res> {
  _$LibraryEvent_TracksCopyWithImpl(this._self, this._then);

  final LibraryEvent_Tracks _self;
  final $Res Function(LibraryEvent_Tracks) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? folder = null,Object? recursive = null,Object? query = null,Object? items = null,}) {
  return _then(LibraryEvent_Tracks(
folder: null == folder ? _self.folder : folder // ignore: cast_nullable_to_non_nullable
as String,recursive: null == recursive ? _self.recursive : recursive // ignore: cast_nullable_to_non_nullable
as bool,query: null == query ? _self.query : query // ignore: cast_nullable_to_non_nullable
as String,items: null == items ? _self._items : items // ignore: cast_nullable_to_non_nullable
as List<TrackLite>,
  ));
}


}

/// @nodoc


class LibraryEvent_ScanProgress extends LibraryEvent {
  const LibraryEvent_ScanProgress({required this.scanned, required this.updated, required this.skipped, required this.errors}): super._();
  

 final  PlatformInt64 scanned;
 final  PlatformInt64 updated;
 final  PlatformInt64 skipped;
 final  PlatformInt64 errors;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_ScanProgressCopyWith<LibraryEvent_ScanProgress> get copyWith => _$LibraryEvent_ScanProgressCopyWithImpl<LibraryEvent_ScanProgress>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_ScanProgress&&(identical(other.scanned, scanned) || other.scanned == scanned)&&(identical(other.updated, updated) || other.updated == updated)&&(identical(other.skipped, skipped) || other.skipped == skipped)&&(identical(other.errors, errors) || other.errors == errors));
}


@override
int get hashCode => Object.hash(runtimeType,scanned,updated,skipped,errors);

@override
String toString() {
  return 'LibraryEvent.scanProgress(scanned: $scanned, updated: $updated, skipped: $skipped, errors: $errors)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_ScanProgressCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_ScanProgressCopyWith(LibraryEvent_ScanProgress value, $Res Function(LibraryEvent_ScanProgress) _then) = _$LibraryEvent_ScanProgressCopyWithImpl;
@useResult
$Res call({
 PlatformInt64 scanned, PlatformInt64 updated, PlatformInt64 skipped, PlatformInt64 errors
});




}
/// @nodoc
class _$LibraryEvent_ScanProgressCopyWithImpl<$Res>
    implements $LibraryEvent_ScanProgressCopyWith<$Res> {
  _$LibraryEvent_ScanProgressCopyWithImpl(this._self, this._then);

  final LibraryEvent_ScanProgress _self;
  final $Res Function(LibraryEvent_ScanProgress) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? scanned = null,Object? updated = null,Object? skipped = null,Object? errors = null,}) {
  return _then(LibraryEvent_ScanProgress(
scanned: null == scanned ? _self.scanned : scanned // ignore: cast_nullable_to_non_nullable
as PlatformInt64,updated: null == updated ? _self.updated : updated // ignore: cast_nullable_to_non_nullable
as PlatformInt64,skipped: null == skipped ? _self.skipped : skipped // ignore: cast_nullable_to_non_nullable
as PlatformInt64,errors: null == errors ? _self.errors : errors // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class LibraryEvent_ScanFinished extends LibraryEvent {
  const LibraryEvent_ScanFinished({required this.durationMs, required this.scanned, required this.updated, required this.skipped, required this.errors}): super._();
  

 final  PlatformInt64 durationMs;
 final  PlatformInt64 scanned;
 final  PlatformInt64 updated;
 final  PlatformInt64 skipped;
 final  PlatformInt64 errors;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_ScanFinishedCopyWith<LibraryEvent_ScanFinished> get copyWith => _$LibraryEvent_ScanFinishedCopyWithImpl<LibraryEvent_ScanFinished>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_ScanFinished&&(identical(other.durationMs, durationMs) || other.durationMs == durationMs)&&(identical(other.scanned, scanned) || other.scanned == scanned)&&(identical(other.updated, updated) || other.updated == updated)&&(identical(other.skipped, skipped) || other.skipped == skipped)&&(identical(other.errors, errors) || other.errors == errors));
}


@override
int get hashCode => Object.hash(runtimeType,durationMs,scanned,updated,skipped,errors);

@override
String toString() {
  return 'LibraryEvent.scanFinished(durationMs: $durationMs, scanned: $scanned, updated: $updated, skipped: $skipped, errors: $errors)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_ScanFinishedCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_ScanFinishedCopyWith(LibraryEvent_ScanFinished value, $Res Function(LibraryEvent_ScanFinished) _then) = _$LibraryEvent_ScanFinishedCopyWithImpl;
@useResult
$Res call({
 PlatformInt64 durationMs, PlatformInt64 scanned, PlatformInt64 updated, PlatformInt64 skipped, PlatformInt64 errors
});




}
/// @nodoc
class _$LibraryEvent_ScanFinishedCopyWithImpl<$Res>
    implements $LibraryEvent_ScanFinishedCopyWith<$Res> {
  _$LibraryEvent_ScanFinishedCopyWithImpl(this._self, this._then);

  final LibraryEvent_ScanFinished _self;
  final $Res Function(LibraryEvent_ScanFinished) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? durationMs = null,Object? scanned = null,Object? updated = null,Object? skipped = null,Object? errors = null,}) {
  return _then(LibraryEvent_ScanFinished(
durationMs: null == durationMs ? _self.durationMs : durationMs // ignore: cast_nullable_to_non_nullable
as PlatformInt64,scanned: null == scanned ? _self.scanned : scanned // ignore: cast_nullable_to_non_nullable
as PlatformInt64,updated: null == updated ? _self.updated : updated // ignore: cast_nullable_to_non_nullable
as PlatformInt64,skipped: null == skipped ? _self.skipped : skipped // ignore: cast_nullable_to_non_nullable
as PlatformInt64,errors: null == errors ? _self.errors : errors // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class LibraryEvent_SearchResult extends LibraryEvent {
  const LibraryEvent_SearchResult({required this.query, required final  List<TrackLite> items}): _items = items,super._();
  

 final  String query;
 final  List<TrackLite> _items;
 List<TrackLite> get items {
  if (_items is EqualUnmodifiableListView) return _items;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_items);
}


/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_SearchResultCopyWith<LibraryEvent_SearchResult> get copyWith => _$LibraryEvent_SearchResultCopyWithImpl<LibraryEvent_SearchResult>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_SearchResult&&(identical(other.query, query) || other.query == query)&&const DeepCollectionEquality().equals(other._items, _items));
}


@override
int get hashCode => Object.hash(runtimeType,query,const DeepCollectionEquality().hash(_items));

@override
String toString() {
  return 'LibraryEvent.searchResult(query: $query, items: $items)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_SearchResultCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_SearchResultCopyWith(LibraryEvent_SearchResult value, $Res Function(LibraryEvent_SearchResult) _then) = _$LibraryEvent_SearchResultCopyWithImpl;
@useResult
$Res call({
 String query, List<TrackLite> items
});




}
/// @nodoc
class _$LibraryEvent_SearchResultCopyWithImpl<$Res>
    implements $LibraryEvent_SearchResultCopyWith<$Res> {
  _$LibraryEvent_SearchResultCopyWithImpl(this._self, this._then);

  final LibraryEvent_SearchResult _self;
  final $Res Function(LibraryEvent_SearchResult) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? query = null,Object? items = null,}) {
  return _then(LibraryEvent_SearchResult(
query: null == query ? _self.query : query // ignore: cast_nullable_to_non_nullable
as String,items: null == items ? _self._items : items // ignore: cast_nullable_to_non_nullable
as List<TrackLite>,
  ));
}


}

/// @nodoc


class LibraryEvent_Playlists extends LibraryEvent {
  const LibraryEvent_Playlists({required final  List<PlaylistLite> items}): _items = items,super._();
  

 final  List<PlaylistLite> _items;
 List<PlaylistLite> get items {
  if (_items is EqualUnmodifiableListView) return _items;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_items);
}


/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_PlaylistsCopyWith<LibraryEvent_Playlists> get copyWith => _$LibraryEvent_PlaylistsCopyWithImpl<LibraryEvent_Playlists>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_Playlists&&const DeepCollectionEquality().equals(other._items, _items));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(_items));

@override
String toString() {
  return 'LibraryEvent.playlists(items: $items)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_PlaylistsCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_PlaylistsCopyWith(LibraryEvent_Playlists value, $Res Function(LibraryEvent_Playlists) _then) = _$LibraryEvent_PlaylistsCopyWithImpl;
@useResult
$Res call({
 List<PlaylistLite> items
});




}
/// @nodoc
class _$LibraryEvent_PlaylistsCopyWithImpl<$Res>
    implements $LibraryEvent_PlaylistsCopyWith<$Res> {
  _$LibraryEvent_PlaylistsCopyWithImpl(this._self, this._then);

  final LibraryEvent_Playlists _self;
  final $Res Function(LibraryEvent_Playlists) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? items = null,}) {
  return _then(LibraryEvent_Playlists(
items: null == items ? _self._items : items // ignore: cast_nullable_to_non_nullable
as List<PlaylistLite>,
  ));
}


}

/// @nodoc


class LibraryEvent_PlaylistTracks extends LibraryEvent {
  const LibraryEvent_PlaylistTracks({required this.playlistId, required this.query, required final  List<TrackLite> items}): _items = items,super._();
  

 final  PlatformInt64 playlistId;
 final  String query;
 final  List<TrackLite> _items;
 List<TrackLite> get items {
  if (_items is EqualUnmodifiableListView) return _items;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_items);
}


/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_PlaylistTracksCopyWith<LibraryEvent_PlaylistTracks> get copyWith => _$LibraryEvent_PlaylistTracksCopyWithImpl<LibraryEvent_PlaylistTracks>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_PlaylistTracks&&(identical(other.playlistId, playlistId) || other.playlistId == playlistId)&&(identical(other.query, query) || other.query == query)&&const DeepCollectionEquality().equals(other._items, _items));
}


@override
int get hashCode => Object.hash(runtimeType,playlistId,query,const DeepCollectionEquality().hash(_items));

@override
String toString() {
  return 'LibraryEvent.playlistTracks(playlistId: $playlistId, query: $query, items: $items)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_PlaylistTracksCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_PlaylistTracksCopyWith(LibraryEvent_PlaylistTracks value, $Res Function(LibraryEvent_PlaylistTracks) _then) = _$LibraryEvent_PlaylistTracksCopyWithImpl;
@useResult
$Res call({
 PlatformInt64 playlistId, String query, List<TrackLite> items
});




}
/// @nodoc
class _$LibraryEvent_PlaylistTracksCopyWithImpl<$Res>
    implements $LibraryEvent_PlaylistTracksCopyWith<$Res> {
  _$LibraryEvent_PlaylistTracksCopyWithImpl(this._self, this._then);

  final LibraryEvent_PlaylistTracks _self;
  final $Res Function(LibraryEvent_PlaylistTracks) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? playlistId = null,Object? query = null,Object? items = null,}) {
  return _then(LibraryEvent_PlaylistTracks(
playlistId: null == playlistId ? _self.playlistId : playlistId // ignore: cast_nullable_to_non_nullable
as PlatformInt64,query: null == query ? _self.query : query // ignore: cast_nullable_to_non_nullable
as String,items: null == items ? _self._items : items // ignore: cast_nullable_to_non_nullable
as List<TrackLite>,
  ));
}


}

/// @nodoc


class LibraryEvent_LikedTrackIds extends LibraryEvent {
  const LibraryEvent_LikedTrackIds({required this.trackIds}): super._();
  

 final  Int64List trackIds;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_LikedTrackIdsCopyWith<LibraryEvent_LikedTrackIds> get copyWith => _$LibraryEvent_LikedTrackIdsCopyWithImpl<LibraryEvent_LikedTrackIds>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_LikedTrackIds&&const DeepCollectionEquality().equals(other.trackIds, trackIds));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(trackIds));

@override
String toString() {
  return 'LibraryEvent.likedTrackIds(trackIds: $trackIds)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_LikedTrackIdsCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_LikedTrackIdsCopyWith(LibraryEvent_LikedTrackIds value, $Res Function(LibraryEvent_LikedTrackIds) _then) = _$LibraryEvent_LikedTrackIdsCopyWithImpl;
@useResult
$Res call({
 Int64List trackIds
});




}
/// @nodoc
class _$LibraryEvent_LikedTrackIdsCopyWithImpl<$Res>
    implements $LibraryEvent_LikedTrackIdsCopyWith<$Res> {
  _$LibraryEvent_LikedTrackIdsCopyWithImpl(this._self, this._then);

  final LibraryEvent_LikedTrackIds _self;
  final $Res Function(LibraryEvent_LikedTrackIds) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? trackIds = null,}) {
  return _then(LibraryEvent_LikedTrackIds(
trackIds: null == trackIds ? _self.trackIds : trackIds // ignore: cast_nullable_to_non_nullable
as Int64List,
  ));
}


}

/// @nodoc


class LibraryEvent_Error extends LibraryEvent {
  const LibraryEvent_Error({required this.message}): super._();
  

 final  String message;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_ErrorCopyWith<LibraryEvent_Error> get copyWith => _$LibraryEvent_ErrorCopyWithImpl<LibraryEvent_Error>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_Error&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'LibraryEvent.error(message: $message)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_ErrorCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_ErrorCopyWith(LibraryEvent_Error value, $Res Function(LibraryEvent_Error) _then) = _$LibraryEvent_ErrorCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$LibraryEvent_ErrorCopyWithImpl<$Res>
    implements $LibraryEvent_ErrorCopyWith<$Res> {
  _$LibraryEvent_ErrorCopyWithImpl(this._self, this._then);

  final LibraryEvent_Error _self;
  final $Res Function(LibraryEvent_Error) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(LibraryEvent_Error(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class LibraryEvent_Log extends LibraryEvent {
  const LibraryEvent_Log({required this.message}): super._();
  

 final  String message;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LibraryEvent_LogCopyWith<LibraryEvent_Log> get copyWith => _$LibraryEvent_LogCopyWithImpl<LibraryEvent_Log>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LibraryEvent_Log&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'LibraryEvent.log(message: $message)';
}


}

/// @nodoc
abstract mixin class $LibraryEvent_LogCopyWith<$Res> implements $LibraryEventCopyWith<$Res> {
  factory $LibraryEvent_LogCopyWith(LibraryEvent_Log value, $Res Function(LibraryEvent_Log) _then) = _$LibraryEvent_LogCopyWithImpl;
@useResult
$Res call({
 String message
});




}
/// @nodoc
class _$LibraryEvent_LogCopyWithImpl<$Res>
    implements $LibraryEvent_LogCopyWith<$Res> {
  _$LibraryEvent_LogCopyWithImpl(this._self, this._then);

  final LibraryEvent_Log _self;
  final $Res Function(LibraryEvent_Log) _then;

/// Create a copy of LibraryEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(LibraryEvent_Log(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$LyricsEvent {

 String get trackKey;
/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LyricsEventCopyWith<LyricsEvent> get copyWith => _$LyricsEventCopyWithImpl<LyricsEvent>(this as LyricsEvent, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LyricsEvent&&(identical(other.trackKey, trackKey) || other.trackKey == trackKey));
}


@override
int get hashCode => Object.hash(runtimeType,trackKey);

@override
String toString() {
  return 'LyricsEvent(trackKey: $trackKey)';
}


}

/// @nodoc
abstract mixin class $LyricsEventCopyWith<$Res>  {
  factory $LyricsEventCopyWith(LyricsEvent value, $Res Function(LyricsEvent) _then) = _$LyricsEventCopyWithImpl;
@useResult
$Res call({
 String trackKey
});




}
/// @nodoc
class _$LyricsEventCopyWithImpl<$Res>
    implements $LyricsEventCopyWith<$Res> {
  _$LyricsEventCopyWithImpl(this._self, this._then);

  final LyricsEvent _self;
  final $Res Function(LyricsEvent) _then;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') @override $Res call({Object? trackKey = null,}) {
  return _then(_self.copyWith(
trackKey: null == trackKey ? _self.trackKey : trackKey // ignore: cast_nullable_to_non_nullable
as String,
  ));
}

}


/// Adds pattern-matching-related methods to [LyricsEvent].
extension LyricsEventPatterns on LyricsEvent {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( LyricsEvent_Loading value)?  loading,TResult Function( LyricsEvent_Ready value)?  ready,TResult Function( LyricsEvent_Cursor value)?  cursor,TResult Function( LyricsEvent_Empty value)?  empty,TResult Function( LyricsEvent_Error value)?  error,required TResult orElse(),}){
final _that = this;
switch (_that) {
case LyricsEvent_Loading() when loading != null:
return loading(_that);case LyricsEvent_Ready() when ready != null:
return ready(_that);case LyricsEvent_Cursor() when cursor != null:
return cursor(_that);case LyricsEvent_Empty() when empty != null:
return empty(_that);case LyricsEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( LyricsEvent_Loading value)  loading,required TResult Function( LyricsEvent_Ready value)  ready,required TResult Function( LyricsEvent_Cursor value)  cursor,required TResult Function( LyricsEvent_Empty value)  empty,required TResult Function( LyricsEvent_Error value)  error,}){
final _that = this;
switch (_that) {
case LyricsEvent_Loading():
return loading(_that);case LyricsEvent_Ready():
return ready(_that);case LyricsEvent_Cursor():
return cursor(_that);case LyricsEvent_Empty():
return empty(_that);case LyricsEvent_Error():
return error(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( LyricsEvent_Loading value)?  loading,TResult? Function( LyricsEvent_Ready value)?  ready,TResult? Function( LyricsEvent_Cursor value)?  cursor,TResult? Function( LyricsEvent_Empty value)?  empty,TResult? Function( LyricsEvent_Error value)?  error,}){
final _that = this;
switch (_that) {
case LyricsEvent_Loading() when loading != null:
return loading(_that);case LyricsEvent_Ready() when ready != null:
return ready(_that);case LyricsEvent_Cursor() when cursor != null:
return cursor(_that);case LyricsEvent_Empty() when empty != null:
return empty(_that);case LyricsEvent_Error() when error != null:
return error(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String trackKey)?  loading,TResult Function( String trackKey,  LyricsDoc doc)?  ready,TResult Function( String trackKey,  PlatformInt64 lineIndex)?  cursor,TResult Function( String trackKey)?  empty,TResult Function( String trackKey,  String message)?  error,required TResult orElse(),}) {final _that = this;
switch (_that) {
case LyricsEvent_Loading() when loading != null:
return loading(_that.trackKey);case LyricsEvent_Ready() when ready != null:
return ready(_that.trackKey,_that.doc);case LyricsEvent_Cursor() when cursor != null:
return cursor(_that.trackKey,_that.lineIndex);case LyricsEvent_Empty() when empty != null:
return empty(_that.trackKey);case LyricsEvent_Error() when error != null:
return error(_that.trackKey,_that.message);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String trackKey)  loading,required TResult Function( String trackKey,  LyricsDoc doc)  ready,required TResult Function( String trackKey,  PlatformInt64 lineIndex)  cursor,required TResult Function( String trackKey)  empty,required TResult Function( String trackKey,  String message)  error,}) {final _that = this;
switch (_that) {
case LyricsEvent_Loading():
return loading(_that.trackKey);case LyricsEvent_Ready():
return ready(_that.trackKey,_that.doc);case LyricsEvent_Cursor():
return cursor(_that.trackKey,_that.lineIndex);case LyricsEvent_Empty():
return empty(_that.trackKey);case LyricsEvent_Error():
return error(_that.trackKey,_that.message);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String trackKey)?  loading,TResult? Function( String trackKey,  LyricsDoc doc)?  ready,TResult? Function( String trackKey,  PlatformInt64 lineIndex)?  cursor,TResult? Function( String trackKey)?  empty,TResult? Function( String trackKey,  String message)?  error,}) {final _that = this;
switch (_that) {
case LyricsEvent_Loading() when loading != null:
return loading(_that.trackKey);case LyricsEvent_Ready() when ready != null:
return ready(_that.trackKey,_that.doc);case LyricsEvent_Cursor() when cursor != null:
return cursor(_that.trackKey,_that.lineIndex);case LyricsEvent_Empty() when empty != null:
return empty(_that.trackKey);case LyricsEvent_Error() when error != null:
return error(_that.trackKey,_that.message);case _:
  return null;

}
}

}

/// @nodoc


class LyricsEvent_Loading extends LyricsEvent {
  const LyricsEvent_Loading({required this.trackKey}): super._();
  

@override final  String trackKey;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LyricsEvent_LoadingCopyWith<LyricsEvent_Loading> get copyWith => _$LyricsEvent_LoadingCopyWithImpl<LyricsEvent_Loading>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LyricsEvent_Loading&&(identical(other.trackKey, trackKey) || other.trackKey == trackKey));
}


@override
int get hashCode => Object.hash(runtimeType,trackKey);

@override
String toString() {
  return 'LyricsEvent.loading(trackKey: $trackKey)';
}


}

/// @nodoc
abstract mixin class $LyricsEvent_LoadingCopyWith<$Res> implements $LyricsEventCopyWith<$Res> {
  factory $LyricsEvent_LoadingCopyWith(LyricsEvent_Loading value, $Res Function(LyricsEvent_Loading) _then) = _$LyricsEvent_LoadingCopyWithImpl;
@override @useResult
$Res call({
 String trackKey
});




}
/// @nodoc
class _$LyricsEvent_LoadingCopyWithImpl<$Res>
    implements $LyricsEvent_LoadingCopyWith<$Res> {
  _$LyricsEvent_LoadingCopyWithImpl(this._self, this._then);

  final LyricsEvent_Loading _self;
  final $Res Function(LyricsEvent_Loading) _then;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? trackKey = null,}) {
  return _then(LyricsEvent_Loading(
trackKey: null == trackKey ? _self.trackKey : trackKey // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class LyricsEvent_Ready extends LyricsEvent {
  const LyricsEvent_Ready({required this.trackKey, required this.doc}): super._();
  

@override final  String trackKey;
 final  LyricsDoc doc;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LyricsEvent_ReadyCopyWith<LyricsEvent_Ready> get copyWith => _$LyricsEvent_ReadyCopyWithImpl<LyricsEvent_Ready>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LyricsEvent_Ready&&(identical(other.trackKey, trackKey) || other.trackKey == trackKey)&&(identical(other.doc, doc) || other.doc == doc));
}


@override
int get hashCode => Object.hash(runtimeType,trackKey,doc);

@override
String toString() {
  return 'LyricsEvent.ready(trackKey: $trackKey, doc: $doc)';
}


}

/// @nodoc
abstract mixin class $LyricsEvent_ReadyCopyWith<$Res> implements $LyricsEventCopyWith<$Res> {
  factory $LyricsEvent_ReadyCopyWith(LyricsEvent_Ready value, $Res Function(LyricsEvent_Ready) _then) = _$LyricsEvent_ReadyCopyWithImpl;
@override @useResult
$Res call({
 String trackKey, LyricsDoc doc
});




}
/// @nodoc
class _$LyricsEvent_ReadyCopyWithImpl<$Res>
    implements $LyricsEvent_ReadyCopyWith<$Res> {
  _$LyricsEvent_ReadyCopyWithImpl(this._self, this._then);

  final LyricsEvent_Ready _self;
  final $Res Function(LyricsEvent_Ready) _then;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? trackKey = null,Object? doc = null,}) {
  return _then(LyricsEvent_Ready(
trackKey: null == trackKey ? _self.trackKey : trackKey // ignore: cast_nullable_to_non_nullable
as String,doc: null == doc ? _self.doc : doc // ignore: cast_nullable_to_non_nullable
as LyricsDoc,
  ));
}


}

/// @nodoc


class LyricsEvent_Cursor extends LyricsEvent {
  const LyricsEvent_Cursor({required this.trackKey, required this.lineIndex}): super._();
  

@override final  String trackKey;
 final  PlatformInt64 lineIndex;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LyricsEvent_CursorCopyWith<LyricsEvent_Cursor> get copyWith => _$LyricsEvent_CursorCopyWithImpl<LyricsEvent_Cursor>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LyricsEvent_Cursor&&(identical(other.trackKey, trackKey) || other.trackKey == trackKey)&&(identical(other.lineIndex, lineIndex) || other.lineIndex == lineIndex));
}


@override
int get hashCode => Object.hash(runtimeType,trackKey,lineIndex);

@override
String toString() {
  return 'LyricsEvent.cursor(trackKey: $trackKey, lineIndex: $lineIndex)';
}


}

/// @nodoc
abstract mixin class $LyricsEvent_CursorCopyWith<$Res> implements $LyricsEventCopyWith<$Res> {
  factory $LyricsEvent_CursorCopyWith(LyricsEvent_Cursor value, $Res Function(LyricsEvent_Cursor) _then) = _$LyricsEvent_CursorCopyWithImpl;
@override @useResult
$Res call({
 String trackKey, PlatformInt64 lineIndex
});




}
/// @nodoc
class _$LyricsEvent_CursorCopyWithImpl<$Res>
    implements $LyricsEvent_CursorCopyWith<$Res> {
  _$LyricsEvent_CursorCopyWithImpl(this._self, this._then);

  final LyricsEvent_Cursor _self;
  final $Res Function(LyricsEvent_Cursor) _then;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? trackKey = null,Object? lineIndex = null,}) {
  return _then(LyricsEvent_Cursor(
trackKey: null == trackKey ? _self.trackKey : trackKey // ignore: cast_nullable_to_non_nullable
as String,lineIndex: null == lineIndex ? _self.lineIndex : lineIndex // ignore: cast_nullable_to_non_nullable
as PlatformInt64,
  ));
}


}

/// @nodoc


class LyricsEvent_Empty extends LyricsEvent {
  const LyricsEvent_Empty({required this.trackKey}): super._();
  

@override final  String trackKey;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LyricsEvent_EmptyCopyWith<LyricsEvent_Empty> get copyWith => _$LyricsEvent_EmptyCopyWithImpl<LyricsEvent_Empty>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LyricsEvent_Empty&&(identical(other.trackKey, trackKey) || other.trackKey == trackKey));
}


@override
int get hashCode => Object.hash(runtimeType,trackKey);

@override
String toString() {
  return 'LyricsEvent.empty(trackKey: $trackKey)';
}


}

/// @nodoc
abstract mixin class $LyricsEvent_EmptyCopyWith<$Res> implements $LyricsEventCopyWith<$Res> {
  factory $LyricsEvent_EmptyCopyWith(LyricsEvent_Empty value, $Res Function(LyricsEvent_Empty) _then) = _$LyricsEvent_EmptyCopyWithImpl;
@override @useResult
$Res call({
 String trackKey
});




}
/// @nodoc
class _$LyricsEvent_EmptyCopyWithImpl<$Res>
    implements $LyricsEvent_EmptyCopyWith<$Res> {
  _$LyricsEvent_EmptyCopyWithImpl(this._self, this._then);

  final LyricsEvent_Empty _self;
  final $Res Function(LyricsEvent_Empty) _then;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? trackKey = null,}) {
  return _then(LyricsEvent_Empty(
trackKey: null == trackKey ? _self.trackKey : trackKey // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class LyricsEvent_Error extends LyricsEvent {
  const LyricsEvent_Error({required this.trackKey, required this.message}): super._();
  

@override final  String trackKey;
 final  String message;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LyricsEvent_ErrorCopyWith<LyricsEvent_Error> get copyWith => _$LyricsEvent_ErrorCopyWithImpl<LyricsEvent_Error>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LyricsEvent_Error&&(identical(other.trackKey, trackKey) || other.trackKey == trackKey)&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,trackKey,message);

@override
String toString() {
  return 'LyricsEvent.error(trackKey: $trackKey, message: $message)';
}


}

/// @nodoc
abstract mixin class $LyricsEvent_ErrorCopyWith<$Res> implements $LyricsEventCopyWith<$Res> {
  factory $LyricsEvent_ErrorCopyWith(LyricsEvent_Error value, $Res Function(LyricsEvent_Error) _then) = _$LyricsEvent_ErrorCopyWithImpl;
@override @useResult
$Res call({
 String trackKey, String message
});




}
/// @nodoc
class _$LyricsEvent_ErrorCopyWithImpl<$Res>
    implements $LyricsEvent_ErrorCopyWith<$Res> {
  _$LyricsEvent_ErrorCopyWithImpl(this._self, this._then);

  final LyricsEvent_Error _self;
  final $Res Function(LyricsEvent_Error) _then;

/// Create a copy of LyricsEvent
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? trackKey = null,Object? message = null,}) {
  return _then(LyricsEvent_Error(
trackKey: null == trackKey ? _self.trackKey : trackKey // ignore: cast_nullable_to_non_nullable
as String,message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
