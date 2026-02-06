import 'package:flutter/foundation.dart';
import 'package:stellatune/bridge/bridge.dart';

enum LyricsStatus { idle, loading, ready, empty, error }

@immutable
class LyricsState {
  const LyricsState({
    required this.enabled,
    required this.status,
    required this.trackKey,
    required this.doc,
    required this.currentLineIndex,
    required this.lastError,
  });

  const LyricsState.initial()
    : enabled = true,
      status = LyricsStatus.idle,
      trackKey = null,
      doc = null,
      currentLineIndex = -1,
      lastError = null;

  static const Object _sentinel = Object();

  final bool enabled;
  final LyricsStatus status;
  final String? trackKey;
  final LyricsDoc? doc;
  final int currentLineIndex;
  final String? lastError;

  bool get hasLyrics => (doc?.lines.isNotEmpty ?? false);
  bool get isLoading => status == LyricsStatus.loading;
  List<LyricLine> get lines => doc?.lines ?? const <LyricLine>[];

  LyricsState copyWith({
    bool? enabled,
    LyricsStatus? status,
    Object? trackKey = _sentinel,
    Object? doc = _sentinel,
    int? currentLineIndex,
    Object? lastError = _sentinel,
  }) {
    return LyricsState(
      enabled: enabled ?? this.enabled,
      status: status ?? this.status,
      trackKey: identical(trackKey, _sentinel)
          ? this.trackKey
          : trackKey as String?,
      doc: identical(doc, _sentinel) ? this.doc : doc as LyricsDoc?,
      currentLineIndex: currentLineIndex ?? this.currentLineIndex,
      lastError: identical(lastError, _sentinel)
          ? this.lastError
          : lastError as String?,
    );
  }
}
