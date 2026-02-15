// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'lyrics_types.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
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
