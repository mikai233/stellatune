// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'stellatune_library.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( LibraryEvent_Changed value)?  changed,TResult Function( LibraryEvent_ScanProgress value)?  scanProgress,TResult Function( LibraryEvent_ScanFinished value)?  scanFinished,TResult Function( LibraryEvent_Error value)?  error,TResult Function( LibraryEvent_Log value)?  log,required TResult orElse(),}){
final _that = this;
switch (_that) {
case LibraryEvent_Changed() when changed != null:
return changed(_that);case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that);case LibraryEvent_Error() when error != null:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( LibraryEvent_Changed value)  changed,required TResult Function( LibraryEvent_ScanProgress value)  scanProgress,required TResult Function( LibraryEvent_ScanFinished value)  scanFinished,required TResult Function( LibraryEvent_Error value)  error,required TResult Function( LibraryEvent_Log value)  log,}){
final _that = this;
switch (_that) {
case LibraryEvent_Changed():
return changed(_that);case LibraryEvent_ScanProgress():
return scanProgress(_that);case LibraryEvent_ScanFinished():
return scanFinished(_that);case LibraryEvent_Error():
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( LibraryEvent_Changed value)?  changed,TResult? Function( LibraryEvent_ScanProgress value)?  scanProgress,TResult? Function( LibraryEvent_ScanFinished value)?  scanFinished,TResult? Function( LibraryEvent_Error value)?  error,TResult? Function( LibraryEvent_Log value)?  log,}){
final _that = this;
switch (_that) {
case LibraryEvent_Changed() when changed != null:
return changed(_that);case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that);case LibraryEvent_Error() when error != null:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  changed,TResult Function( PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanProgress,TResult Function( PlatformInt64 durationMs,  PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanFinished,TResult Function( String message)?  error,TResult Function( String message)?  log,required TResult orElse(),}) {final _that = this;
switch (_that) {
case LibraryEvent_Changed() when changed != null:
return changed();case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that.durationMs,_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_Error() when error != null:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  changed,required TResult Function( PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)  scanProgress,required TResult Function( PlatformInt64 durationMs,  PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)  scanFinished,required TResult Function( String message)  error,required TResult Function( String message)  log,}) {final _that = this;
switch (_that) {
case LibraryEvent_Changed():
return changed();case LibraryEvent_ScanProgress():
return scanProgress(_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_ScanFinished():
return scanFinished(_that.durationMs,_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_Error():
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  changed,TResult? Function( PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanProgress,TResult? Function( PlatformInt64 durationMs,  PlatformInt64 scanned,  PlatformInt64 updated,  PlatformInt64 skipped,  PlatformInt64 errors)?  scanFinished,TResult? Function( String message)?  error,TResult? Function( String message)?  log,}) {final _that = this;
switch (_that) {
case LibraryEvent_Changed() when changed != null:
return changed();case LibraryEvent_ScanProgress() when scanProgress != null:
return scanProgress(_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_ScanFinished() when scanFinished != null:
return scanFinished(_that.durationMs,_that.scanned,_that.updated,_that.skipped,_that.errors);case LibraryEvent_Error() when error != null:
return error(_that.message);case LibraryEvent_Log() when log != null:
return log(_that.message);case _:
  return null;

}
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

// dart format on
