import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

class NowPlayingBar extends ConsumerWidget {
  const NowPlayingBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);

    final currentTitle = queue.currentItem?.displayTitle ?? '(none)';
    final currentSubtitle = playback.currentPath ?? '';

    return Material(
      elevation: 2,
      color: theme.colorScheme.surfaceContainer,
      child: SizedBox(
        height: 72,
        child: Row(
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
              tooltip: 'Previous',
              onPressed: () =>
                  ref.read(playbackControllerProvider.notifier).previous(),
              icon: const Icon(Icons.skip_previous),
            ),
            IconButton(
              tooltip: 'Play',
              onPressed: () =>
                  ref.read(playbackControllerProvider.notifier).play(),
              icon: const Icon(Icons.play_arrow),
            ),
            IconButton(
              tooltip: 'Pause',
              onPressed: () =>
                  ref.read(playbackControllerProvider.notifier).pause(),
              icon: const Icon(Icons.pause),
            ),
            IconButton(
              tooltip: 'Stop',
              onPressed: () =>
                  ref.read(playbackControllerProvider.notifier).stop(),
              icon: const Icon(Icons.stop),
            ),
            IconButton(
              tooltip: 'Next',
              onPressed: () =>
                  ref.read(playbackControllerProvider.notifier).next(),
              icon: const Icon(Icons.skip_next),
            ),
            const SizedBox(width: 8),
            IconButton(
              tooltip: 'Shuffle',
              onPressed: () =>
                  ref.read(queueControllerProvider.notifier).toggleShuffle(),
              icon: Icon(
                Icons.shuffle,
                color: queue.shuffle ? theme.colorScheme.primary : null,
              ),
            ),
            IconButton(
              tooltip: 'Repeat',
              onPressed: () =>
                  ref.read(queueControllerProvider.notifier).cycleRepeatMode(),
              icon: Icon(
                switch (queue.repeatMode) {
                  RepeatMode.one => Icons.repeat_one,
                  _ => Icons.repeat,
                },
                color: queue.repeatMode == RepeatMode.off
                    ? null
                    : theme.colorScheme.primary,
              ),
            ),
            const SizedBox(width: 12),
          ],
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
