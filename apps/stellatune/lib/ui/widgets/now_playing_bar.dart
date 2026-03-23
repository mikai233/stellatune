import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playability_messages.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/widgets/now_playing_bar/widgets/now_playing_bar_sections.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

class NowPlayingBar extends ConsumerWidget {
  const NowPlayingBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);
    final selectedRenderer = ref.watch(dlnaSelectedRendererProvider);
    final coverDir = ref.watch(coverDirProvider);

    final currentTitle = queue.currentItem?.displayTitle ?? l10n.nowPlayingNone;
    final String currentSubtitle;
    if (queue.currentItem != null) {
      final artist = (queue.currentItem?.artist ?? '').trim();
      final album = (queue.currentItem?.album ?? '').trim();
      currentSubtitle = [artist, album].where((s) => s.isNotEmpty).join(' • ');
    } else {
      currentSubtitle = playback.currentPath ?? '';
    }
    final playModeLabel = switch (queue.playMode) {
      PlayMode.sequential => l10n.playModeSequential,
      PlayMode.shuffle => l10n.playModeShuffle,
      PlayMode.repeatAll => l10n.playModeRepeatAll,
      PlayMode.repeatOne => l10n.playModeRepeatOne,
    };

    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;
    const rightControlButtonSize = 50.0;
    const rightControlIconSize = 24.0;
    final localizedPlaybackError = playback.lastError == null
        ? null
        : localizePlaybackError(l10n, playback.lastError!);
    final totalDurationMs = playback.trackInfo?.durationMs?.toInt();
    final progressEnabled =
        queue.currentItem != null &&
        playback.currentPath != null &&
        playback.currentPath!.isNotEmpty;

    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [
            theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.9),
            theme.colorScheme.surfaceContainer.withValues(alpha: 0.94),
          ],
        ),
        border: Border(
          top: BorderSide(
            color: theme.colorScheme.onSurface.withValues(alpha: 0.08),
          ),
        ),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.08),
            blurRadius: 12,
            offset: const Offset(0, -2),
          ),
        ],
      ),
      child: SizedBox(
        height: 76,
        child: Stack(
          children: [
            Row(
              children: [
                const SizedBox(width: 12),
                Expanded(
                  child: NowPlayingTrackInfoSection(
                    theme: theme,
                    coverDir: coverDir,
                    currentItem: queue.currentItem,
                    currentTitle: currentTitle,
                    currentSubtitle: currentSubtitle,
                    currentPath: playback.currentPath,
                    sampleRate: playback.trackInfo?.sampleRate,
                  ),
                ),
                SizedBox(
                  width: 194,
                  child: NowPlayingTransportControls(
                    l10n: l10n,
                    isPlaying: isPlaying,
                  ),
                ),
                Expanded(
                  child: NowPlayingRightControls(
                    l10n: l10n,
                    theme: theme,
                    playback: playback,
                    queue: queue,
                    selectedRenderer: selectedRenderer,
                    localizedPlaybackError: localizedPlaybackError,
                    playModeLabel: playModeLabel,
                    rightControlButtonSize: rightControlButtonSize,
                    rightControlIconSize: rightControlIconSize,
                  ),
                ),
                const SizedBox(width: 12),
              ],
            ),
            Positioned(
              left: 0,
              right: 0,
              top: 0,
              child: NowPlayingProgressBar(
                durationMs: totalDurationMs,
                positionMs: playback.positionMs,
                enabled: progressEnabled,
                audioStarted: playback.audioStarted,
                playerState: playback.playerState,
                onSeekMs: (ms) =>
                    ref.read(playbackControllerProvider.notifier).seekMs(ms),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
