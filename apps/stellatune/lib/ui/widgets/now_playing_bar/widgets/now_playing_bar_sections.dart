import 'package:animations/animations.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/api/dlna/types.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/music_detail_page.dart';
import 'package:stellatune/ui/widgets/audio_format_badge.dart';
import 'package:stellatune/ui/widgets/marquee_text.dart';
import 'package:stellatune/ui/widgets/now_playing_bar/widgets/now_playing_dlna_dialog.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

class NowPlayingTrackInfoSection extends StatelessWidget {
  const NowPlayingTrackInfoSection({
    super.key,
    required this.theme,
    required this.coverDir,
    required this.currentItem,
    required this.currentTitle,
    required this.currentSubtitle,
    required this.currentPath,
    required this.sampleRate,
  });

  final ThemeData theme;
  final String coverDir;
  final QueueItem? currentItem;
  final String currentTitle;
  final String currentSubtitle;
  final String? currentPath;
  final int? sampleRate;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
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
          closedBuilder: (context, open) => NowPlayingCover(
            coverDir: coverDir,
            trackId: currentItem?.id,
            cover: currentItem?.cover,
            primaryColor: theme.colorScheme.primary,
            onTap: currentItem != null ? open : null,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              MarqueeText(
                text: currentTitle,
                style: theme.textTheme.bodyMedium,
              ),
              if (currentSubtitle.isNotEmpty)
                Row(
                  children: [
                    if (currentPath != null) ...[
                      AudioFormatBadge(
                        path: currentPath!,
                        sampleRate: sampleRate,
                      ),
                      const SizedBox(width: 4),
                    ],
                    Expanded(
                      child: MarqueeText(
                        text: currentSubtitle,
                        style: theme.textTheme.bodySmall,
                      ),
                    ),
                  ],
                ),
            ],
          ),
        ),
      ],
    );
  }
}

class NowPlayingTransportControls extends ConsumerWidget {
  const NowPlayingTransportControls({
    super.key,
    required this.l10n,
    required this.isPlaying,
  });

  final AppLocalizations l10n;
  final bool isPlaying;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        IconButton(
          tooltip: l10n.tooltipPrevious,
          onPressed: () =>
              ref.read(playbackControllerProvider.notifier).previous(),
          iconSize: 30,
          constraints: const BoxConstraints.tightFor(width: 50, height: 50),
          icon: const Icon(Icons.skip_previous),
        ),
        const SizedBox(width: 2),
        IconButton(
          tooltip: isPlaying ? l10n.pause : l10n.play,
          onPressed: () => isPlaying
              ? ref.read(playbackControllerProvider.notifier).pause()
              : ref.read(playbackControllerProvider.notifier).play(),
          iconSize: 38,
          constraints: const BoxConstraints.tightFor(width: 58, height: 58),
          icon: Icon(isPlaying ? Icons.pause : Icons.play_arrow),
        ),
        const SizedBox(width: 2),
        IconButton(
          tooltip: l10n.tooltipNext,
          onPressed: () => ref.read(playbackControllerProvider.notifier).next(),
          iconSize: 30,
          constraints: const BoxConstraints.tightFor(width: 50, height: 50),
          icon: const Icon(Icons.skip_next),
        ),
      ],
    );
  }
}

class NowPlayingRightControls extends ConsumerWidget {
  const NowPlayingRightControls({
    super.key,
    required this.l10n,
    required this.theme,
    required this.playback,
    required this.queue,
    required this.selectedRenderer,
    required this.localizedPlaybackError,
    required this.playModeLabel,
    required this.rightControlButtonSize,
    required this.rightControlIconSize,
  });

  final AppLocalizations l10n;
  final ThemeData theme;
  final PlaybackState playback;
  final QueueState queue;
  final DlnaRenderer? selectedRenderer;
  final String? localizedPlaybackError;
  final String playModeLabel;
  final double rightControlButtonSize;
  final double rightControlIconSize;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        if (localizedPlaybackError != null)
          IconButton(
            tooltip: localizedPlaybackError,
            onPressed: () {
              ScaffoldMessenger.of(
                context,
              ).showSnackBar(SnackBar(content: Text(localizedPlaybackError!)));
            },
            icon: Icon(Icons.error_outline, color: theme.colorScheme.error),
            iconSize: 24,
            constraints: const BoxConstraints.tightFor(width: 46, height: 46),
          ),
        const SizedBox(width: 6),
        VolumePopupButton(
          volume: playback.desiredVolume,
          iconSize: rightControlIconSize,
          buttonSize: rightControlButtonSize,
          enableHover: true,
          onChanged: (v) =>
              ref.read(playbackControllerProvider.notifier).setVolume(v),
          onToggleMute: () =>
              ref.read(playbackControllerProvider.notifier).toggleMute(),
        ),
        IconButton(
          tooltip: playModeLabel,
          onPressed: () =>
              ref.read(queueControllerProvider.notifier).cyclePlayMode(),
          iconSize: rightControlIconSize,
          constraints: const BoxConstraints.tightFor(width: 50, height: 50),
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
        IconButton(
          tooltip: selectedRenderer == null
              ? 'DLNA'
              : 'DLNA: ${selectedRenderer!.friendlyName}',
          onPressed: () async {
            final chosen = await showDialog<DlnaActionResult>(
              context: context,
              builder: (context) => DlnaDialog(selected: selectedRenderer),
            );
            if (chosen == null) return;

            if (chosen.applySelection) {
              ref
                  .read(dlnaSelectedRendererProvider.notifier)
                  .set(chosen.selected);
            }

            final message = chosen.message;
            if (message != null && context.mounted) {
              ScaffoldMessenger.of(
                context,
              ).showSnackBar(SnackBar(content: Text(message)));
            }
          },
          icon: Icon(
            Icons.cast,
            size: 22,
            color: selectedRenderer == null ? null : theme.colorScheme.primary,
          ),
          iconSize: rightControlIconSize,
          constraints: const BoxConstraints.tightFor(width: 50, height: 50),
        ),
      ],
    );
  }
}
