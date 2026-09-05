import 'package:flutter/foundation.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/queue_models.dart';

const Object _playbackStateSentinel = Object();

@immutable
class PlaybackState {
  const PlaybackState({
    required this.playerState,
    required this.positionMs,
    required this.currentPath,
    required this.desiredVolume,
    required this.appliedVolume,
    required this.lastError,
    required this.lastLog,
    required this.audioStarted,
    this.trackInfo,
    this.pendingItem,
  });

  const PlaybackState.initial()
    : playerState = PlayerState.stopped,
      positionMs = 0,
      currentPath = null,
      desiredVolume = 1.0,
      appliedVolume = 1.0,
      lastError = null,
      lastLog = '',
      audioStarted = false,
      trackInfo = null,
      pendingItem = null;

  final PlayerState playerState;
  final int positionMs;
  final String? currentPath;
  final double desiredVolume;
  final double appliedVolume;
  final String? lastError;
  final String lastLog;
  final bool audioStarted;
  final TrackDecodeInfo? trackInfo;
  final QueueItem? pendingItem;

  PlaybackState copyWith({
    PlayerState? playerState,
    int? positionMs,
    String? currentPath,
    double? desiredVolume,
    double? appliedVolume,
    Object? lastError = _playbackStateSentinel,
    String? lastLog,
    bool? audioStarted,
    Object? trackInfo = _playbackStateSentinel,
    Object? pendingItem = _playbackStateSentinel,
  }) {
    return PlaybackState(
      playerState: playerState ?? this.playerState,
      positionMs: positionMs ?? this.positionMs,
      currentPath: currentPath ?? this.currentPath,
      desiredVolume: desiredVolume ?? this.desiredVolume,
      appliedVolume: appliedVolume ?? this.appliedVolume,
      lastError: identical(lastError, _playbackStateSentinel)
          ? this.lastError
          : lastError as String?,
      pendingItem: identical(pendingItem, _playbackStateSentinel)
          ? this.pendingItem
          : pendingItem as QueueItem?,
      lastLog: lastLog ?? this.lastLog,
      audioStarted: audioStarted ?? this.audioStarted,
      trackInfo: identical(trackInfo, _playbackStateSentinel)
          ? this.trackInfo
          : trackInfo as TrackDecodeInfo?,
    );
  }
}
