import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/third_party/stellatune_core.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/ui/pages/music_detail_page.dart';
import 'package:stellatune/ui/widgets/marquee_text.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

class MobileNowPlayingBar extends ConsumerWidget {
  const MobileNowPlayingBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);

    final currentTitle = queue.currentItem?.displayTitle ?? l10n.nowPlayingNone;
    final String currentSubtitle;
    if (queue.currentItem != null) {
      final artist = (queue.currentItem?.artist ?? '').trim();
      currentSubtitle = artist;
    } else {
      currentSubtitle = '';
    }

    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;

    return Material(
      elevation: 4,
      surfaceTintColor: theme.colorScheme.surfaceTint,
      color: theme.colorScheme.surface,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(12)),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            height: 64,
            child: Row(
              children: [
                const SizedBox(width: 8),
                OpenContainer(
                  closedElevation: 0,
                  openElevation: 0,
                  closedColor: Colors.black,
                  openColor: Colors.black,
                  closedShape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(8),
                  ),
                  openShape: const RoundedRectangleBorder(
                    borderRadius: BorderRadius.zero,
                  ),
                  transitionDuration: const Duration(milliseconds: 400),
                  transitionType: ContainerTransitionType.fade,
                  openBuilder: (context, close) => const MusicDetailPage(),
                  closedBuilder: (context, open) => Consumer(
                    builder: (context, ref, child) {
                      final innerQueue = ref.watch(queueControllerProvider);
                      final innerCoverDir = ref.watch(coverDirProvider);
                      final innerTrackId = innerQueue.currentItem?.id;
                      final cover = innerQueue.currentItem?.cover;
                      return Padding(
                        padding: const EdgeInsets.all(4),
                        child: NowPlayingCover(
                          coverDir: innerCoverDir,
                          trackId: innerTrackId,
                          cover: cover,
                          primaryColor: theme.colorScheme.primary,
                          onTap: innerQueue.currentItem != null ? open : null,
                        ),
                      );
                    },
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: GestureDetector(
                    onTap: () {
                      if (queue.currentItem != null) {
                        Navigator.of(context).push(
                          PageRouteBuilder(
                            pageBuilder:
                                (context, animation, secondaryAnimation) =>
                                    const MusicDetailPage(),
                            transitionsBuilder:
                                (
                                  context,
                                  animation,
                                  secondaryAnimation,
                                  child,
                                ) {
                                  return FadeThroughTransition(
                                    animation: animation,
                                    secondaryAnimation: secondaryAnimation,
                                    child: child,
                                  );
                                },
                          ),
                        );
                      }
                    },
                    behavior: HitTestBehavior.translucent,
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        MarqueeText(
                          text: currentTitle,
                          style: theme.textTheme.titleMedium,
                        ),
                        if (currentSubtitle.isNotEmpty)
                          Text(
                            currentSubtitle,
                            style: theme.textTheme.bodyMedium?.copyWith(
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                      ],
                    ),
                  ),
                ),
                IconButton(
                  tooltip: isPlaying ? l10n.pause : l10n.play,
                  onPressed: () => isPlaying
                      ? ref.read(playbackControllerProvider.notifier).pause()
                      : ref.read(playbackControllerProvider.notifier).play(),
                  icon: Icon(isPlaying ? Icons.pause : Icons.play_arrow),
                  style: IconButton.styleFrom(
                    backgroundColor: theme.colorScheme.primaryContainer,
                    foregroundColor: theme.colorScheme.onPrimaryContainer,
                  ),
                ),
                const SizedBox(width: 8),
              ],
            ),
          ),
          NowPlayingProgressBar(
            durationMs: queue.currentItem?.durationMs,
            positionMs: playback.positionMs,
            enabled:
                queue.currentItem != null &&
                playback.currentPath != null &&
                playback.currentPath!.isNotEmpty,
            playerState: playback.playerState,
            onSeekMs: (ms) =>
                ref.read(playbackControllerProvider.notifier).seekMs(ms),
          ),
        ],
      ),
    );
  }
}
