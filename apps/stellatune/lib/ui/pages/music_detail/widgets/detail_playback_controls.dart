import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';

import 'bottom_playback_bar.dart';

/// Playback ticks rebuild this bar without rebuilding artwork or page layout.
class DetailPlaybackControls extends ConsumerWidget {
  const DetailPlaybackControls({
    super.key,
    required this.foregroundColor,
    required this.onQueuePressed,
    required this.enableVolumeHover,
  });

  final Color foregroundColor;
  final VoidCallback onQueuePressed;
  final bool enableVolumeHover;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final playback = ref.watch(playbackControllerProvider);
    final playMode = ref.watch(
      queueControllerProvider.select((queue) => queue.playMode),
    );
    final item = ref.watch(
      queueControllerProvider.select((queue) => queue.currentItem),
    );
    final controller = ref.read(playbackControllerProvider.notifier);
    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;
    return BottomPlaybackBar(
      positionMs: playback.positionMs,
      durationMs: item?.isSegment == true
          ? item!.durationMs ?? 0
          : playback.trackInfo?.durationMs?.toInt() ?? 0,
      isPlaying: isPlaying,
      playMode: playMode,
      volume: playback.desiredVolume,
      foregroundColor: foregroundColor,
      currentPath: playback.currentPath,
      audioStarted: playback.audioStarted,
      sampleRate: playback.trackInfo?.sampleRate,
      onPlayPause: isPlaying ? controller.pause : controller.play,
      onPrevious: controller.previous,
      onNext: controller.next,
      onSeek: controller.seekMs,
      onVolumeChanged: controller.setVolume,
      onToggleMute: controller.toggleMute,
      enableVolumeHover: enableVolumeHover,
      onPlayModeChanged: ref
          .read(queueControllerProvider.notifier)
          .cyclePlayMode,
      onQueuePressed: onQueuePressed,
    );
  }
}
