import 'package:flutter/foundation.dart';
import 'package:stellatune/bridge/bridge.dart';

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
    this.trackInfo,
  });

  const PlaybackState.initial()
    : playerState = PlayerState.stopped,
      positionMs = 0,
      currentPath = null,
      desiredVolume = 1.0,
      appliedVolume = 1.0,
      lastError = null,
      lastLog = '',
      trackInfo = null;

  final PlayerState playerState;
  final int positionMs;
  final String? currentPath;
  final double desiredVolume;
  final double appliedVolume;
  final String? lastError;
  final String lastLog;
  final TrackDecodeInfo? trackInfo;

  PlaybackState copyWith({
    PlayerState? playerState,
    int? positionMs,
    String? currentPath,
    double? desiredVolume,
    double? appliedVolume,
    String? lastError,
    String? lastLog,
    TrackDecodeInfo? trackInfo,
  }) {
    return PlaybackState(
      playerState: playerState ?? this.playerState,
      positionMs: positionMs ?? this.positionMs,
      currentPath: currentPath ?? this.currentPath,
      desiredVolume: desiredVolume ?? this.desiredVolume,
      appliedVolume: appliedVolume ?? this.appliedVolume,
      lastError: lastError ?? this.lastError,
      lastLog: lastLog ?? this.lastLog,
      trackInfo: trackInfo ?? this.trackInfo,
    );
  }
}
