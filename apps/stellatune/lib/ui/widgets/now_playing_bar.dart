import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

class NowPlayingBar extends ConsumerWidget {
  const NowPlayingBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);

    final currentTitle = queue.currentItem?.displayTitle ?? l10n.nowPlayingNone;
    final currentSubtitle = playback.currentPath ?? '';
    final playModeLabel = switch (queue.playMode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };

    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;

    return Material(
      elevation: 2,
      color: theme.colorScheme.surfaceContainer,
      child: SizedBox(
        height: 72,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final showInlineVolume = constraints.maxWidth >= 920;

            return Row(
              children: [
                const SizedBox(width: 12),
                _CoverPlaceholder(color: theme.colorScheme.primary),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        currentTitle,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      if (currentSubtitle.isNotEmpty)
                        Text(
                          currentSubtitle,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.bodySmall,
                        ),
                    ],
                  ),
                ),
                Text(
                  _formatMs(playback.positionMs),
                  style: theme.textTheme.titleMedium,
                ),
                const SizedBox(width: 16),
                IconButton(
                  tooltip: l10n.tooltipPrevious,
                  onPressed: () =>
                      ref.read(playbackControllerProvider.notifier).previous(),
                  icon: const Icon(Icons.skip_previous),
                ),
                IconButton(
                  tooltip: isPlaying ? l10n.pause : l10n.play,
                  onPressed: () => isPlaying
                      ? ref.read(playbackControllerProvider.notifier).pause()
                      : ref.read(playbackControllerProvider.notifier).play(),
                  icon: Icon(isPlaying ? Icons.pause : Icons.play_arrow),
                ),
                IconButton(
                  tooltip: l10n.stop,
                  onPressed: () =>
                      ref.read(playbackControllerProvider.notifier).stop(),
                  icon: const Icon(Icons.stop),
                ),
                IconButton(
                  tooltip: l10n.tooltipNext,
                  onPressed: () =>
                      ref.read(playbackControllerProvider.notifier).next(),
                  icon: const Icon(Icons.skip_next),
                ),
                const SizedBox(width: 8),
                if (showInlineVolume)
                  _VolumeControlInline(
                    volume: playback.volume,
                    onChanged: (v) => ref
                        .read(playbackControllerProvider.notifier)
                        .setVolume(v),
                    tooltip: l10n.tooltipVolume,
                  )
                else
                  IconButton(
                    tooltip: l10n.tooltipVolume,
                    icon: const Icon(Icons.volume_up),
                    onPressed: () async {
                      final controller = ref.read(
                        playbackControllerProvider.notifier,
                      );
                      var v = ref.read(playbackControllerProvider).volume;
                      await showDialog<void>(
                        context: context,
                        builder: (context) => AlertDialog(
                          title: Text(l10n.volume),
                          content: StatefulBuilder(
                            builder: (context, setState) {
                              return Row(
                                children: [
                                  const Icon(Icons.volume_up),
                                  const SizedBox(width: 12),
                                  Expanded(
                                    child: Slider(
                                      value: v,
                                      onChanged: (nv) {
                                        setState(() => v = nv);
                                        controller.setVolume(nv);
                                      },
                                    ),
                                  ),
                                ],
                              );
                            },
                          ),
                          actions: [
                            TextButton(
                              onPressed: () => Navigator.of(context).pop(),
                              child: Text(
                                MaterialLocalizations.of(
                                  context,
                                ).closeButtonLabel,
                              ),
                            ),
                          ],
                        ),
                      );
                    },
                  ),
                IconButton(
                  tooltip: playModeLabel,
                  onPressed: () => ref
                      .read(queueControllerProvider.notifier)
                      .cyclePlayMode(),
                  icon: Icon(
                    switch (queue.playMode) {
                      PlayMode.sequential => Icons.playlist_play,
                      PlayMode.shuffle => Icons.shuffle,
                      PlayMode.repeatAll => Icons.repeat,
                      PlayMode.repeatOne => Icons.repeat_one,
                    },
                    color: queue.playMode == PlayMode.sequential
                        ? null
                        : theme.colorScheme.primary,
                  ),
                ),
                const SizedBox(width: 12),
              ],
            );
          },
        ),
      ),
    );
  }

  static String _formatMs(int ms) {
    final totalSeconds = (ms / 1000).floor();
    final minutes = (totalSeconds / 60).floor();
    final seconds = totalSeconds % 60;
    return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }
}

class _CoverPlaceholder extends StatelessWidget {
  const _CoverPlaceholder({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 48,
      height: 48,
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: color.withValues(alpha: 0.18)),
      ),
      child: Icon(Icons.music_note, color: color),
    );
  }
}

class _VolumeControlInline extends StatelessWidget {
  const _VolumeControlInline({
    required this.volume,
    required this.onChanged,
    required this.tooltip,
  });

  final double volume;
  final ValueChanged<double> onChanged;
  final String tooltip;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Tooltip(message: tooltip, child: const Icon(Icons.volume_up)),
        SizedBox(
          width: 120,
          child: SliderTheme(
            data: SliderTheme.of(context).copyWith(
              trackHeight: 2,
              overlayShape: SliderComponentShape.noOverlay,
            ),
            child: Slider(value: volume.clamp(0.0, 1.0), onChanged: onChanged),
          ),
        ),
        const SizedBox(width: 8),
      ],
    );
  }
}
