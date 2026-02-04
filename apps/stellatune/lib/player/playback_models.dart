import 'package:flutter/foundation.dart';
import 'package:stellatune/bridge/bridge.dart';

@immutable
class PlaybackState {
  const PlaybackState({
    required this.playerState,
    required this.positionMs,
    required this.currentPath,
    required this.volume,
    required this.lastError,
    required this.lastLog,
  });

  const PlaybackState.initial()
    : playerState = PlayerState.stopped,
      positionMs = 0,
      currentPath = null,
      volume = 1.0,
      lastError = null,
      lastLog = '';

  final PlayerState playerState;
  final int positionMs;
  final String? currentPath;
  final double volume;
  final String? lastError;
  final String lastLog;

  PlaybackState copyWith({
    PlayerState? playerState,
    int? positionMs,
    String? currentPath,
    double? volume,
    String? lastError,
    String? lastLog,
  }) {
    return PlaybackState(
      playerState: playerState ?? this.playerState,
      positionMs: positionMs ?? this.positionMs,
      currentPath: currentPath ?? this.currentPath,
      volume: volume ?? this.volume,
      lastError: lastError ?? this.lastError,
      lastLog: lastLog ?? this.lastLog,
    );
  }
}
