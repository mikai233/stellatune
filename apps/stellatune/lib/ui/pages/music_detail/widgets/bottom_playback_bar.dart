import 'dart:io';
import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

/// Bottom playback bar with time, controls, progress slider, and volume.
class BottomPlaybackBar extends StatefulWidget {
  const BottomPlaybackBar({
    super.key,
    required this.positionMs,
    required this.durationMs,
    required this.isPlaying,
    required this.playMode,
    required this.volume,
    required this.onPlayPause,
    required this.onPrevious,
    required this.onNext,
    required this.onSeek,
    required this.onVolumeChanged,
    required this.onPlayModeChanged,
    required this.onQueuePressed,
    required this.foregroundColor,
    required this.onToggleMute,
    required this.audioStarted,
    this.currentPath,
    this.sampleRate,
    this.enableVolumeHover = false,
  });

  final int positionMs;
  final int durationMs;
  final bool isPlaying;
  final PlayMode playMode;
  final double volume;
  final VoidCallback onPlayPause;
  final VoidCallback onPrevious;
  final VoidCallback onNext;
  final ValueChanged<int> onSeek;
  final ValueChanged<double> onVolumeChanged;
  final VoidCallback onPlayModeChanged;
  final VoidCallback onQueuePressed;
  final Color foregroundColor;
  final String? currentPath;
  final int? sampleRate;
  final VoidCallback onToggleMute;
  final bool audioStarted;
  final bool enableVolumeHover;

  @override
  State<BottomPlaybackBar> createState() => _BottomPlaybackBarState();
}

class _BottomPlaybackBarState extends State<BottomPlaybackBar> {
  String _formatMs(int ms) {
    final totalSeconds = ms ~/ 1000;
    final minutes = totalSeconds ~/ 60;
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;

    final playModeIcon = switch (widget.playMode) {
      PlayMode.sequential => Icons.arrow_forward,
      PlayMode.shuffle => Icons.shuffle,
      PlayMode.repeatAll => Icons.repeat,
      PlayMode.repeatOne => Icons.repeat_one,
    };

    final playModeTooltip = switch (widget.playMode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        // Progress
        NowPlayingProgressBar(
          positionMs: widget.positionMs,
          durationMs: widget.durationMs,
          enabled: true,
          audioStarted: widget.audioStarted,
          playerState: widget.isPlaying
              ? PlayerState.playing
              : PlayerState.paused,
          onSeekMs: widget.onSeek,
          foregroundColor: widget.foregroundColor,
          trackHeight: 4.0,
          activeTrackHeight: 6.0,
          barHeight: 16.0,
          activeBarHeight: 16.0,
          showTooltip: true,
        ),
        // Controls row
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Row(
            children: [
              // Left: Time display
              SizedBox(
                width: 60,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      _formatMs(widget.positionMs),
                      style: theme.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w500,
                        color: widget.foregroundColor,
                      ),
                    ),
                    Text(
                      _formatMs(widget.durationMs),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: widget.foregroundColor.withValues(alpha: 0.6),
                      ),
                    ),
                  ],
                ),
              ),
              // Center: Playback controls
              Expanded(
                child: FittedBox(
                  fit: BoxFit.scaleDown,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      if (!Platform.isAndroid && !Platform.isIOS) ...[
                        IconButton(
                          icon: Icon(
                            Icons.equalizer,
                            color: widget.foregroundColor,
                          ),
                          iconSize: 24,
                          onPressed: () {
                            // TODO: Equalizer
                          },
                        ),
                        // Volume button with popup
                        VolumePopupButton(
                          volume: widget.volume,
                          enableHover: widget.enableVolumeHover,
                          onChanged: widget.onVolumeChanged,
                          onToggleMute: widget.onToggleMute,
                          foregroundColor: widget.foregroundColor,
                        ),
                        const SizedBox(width: 8),
                      ],
                      IconButton(
                        icon: Icon(
                          Icons.skip_previous,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 32,
                        tooltip: l10n.tooltipPrevious,
                        onPressed: widget.onPrevious,
                      ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: Icon(
                          widget.isPlaying ? Icons.pause : Icons.play_arrow,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 40,
                        tooltip: widget.isPlaying ? l10n.pause : l10n.play,
                        onPressed: widget.onPlayPause,
                      ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: Icon(
                          Icons.skip_next,
                          color: widget.foregroundColor,
                        ),
                        iconSize: 32,
                        tooltip: l10n.tooltipNext,
                        onPressed: widget.onNext,
                      ),
                      const SizedBox(width: 8),
                      IconButton(
                        icon: Icon(playModeIcon, color: widget.foregroundColor),
                        iconSize: 24,
                        tooltip: playModeTooltip,
                        onPressed: widget.onPlayModeChanged,
                      ),
                      IconButton(
                        icon: Icon(Icons.menu, color: widget.foregroundColor),
                        iconSize: 24,
                        tooltip: l10n.queueTitle,
                        onPressed: widget.onQueuePressed,
                      ),
                    ],
                  ),
                ),
              ),
              // Right: Placeholder for additional controls
              const SizedBox(width: 60),
            ],
          ),
        ),
        const SizedBox(height: 8),
      ],
    );
  }
}
