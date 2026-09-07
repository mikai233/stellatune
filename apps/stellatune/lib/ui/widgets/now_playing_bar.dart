import 'package:stellatune/ui/theme/artwork_palette.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/dlna/dlna_providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/music_detail_page.dart';
import 'package:stellatune/ui/pages/queue_page.dart';
import 'package:stellatune/ui/widgets/audio_format_badge.dart';
import 'package:stellatune/ui/widgets/now_playing_common.dart';

import 'now_playing_bar/desktop_player_bar.dart';
import 'now_playing_bar/widgets/now_playing_dlna_dialog.dart';

class NowPlayingBar extends ConsumerWidget {
  const NowPlayingBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final playback = ref.watch(playbackControllerProvider);
    final queue = ref.watch(queueControllerProvider);
    final renderer = ref.watch(dlnaSelectedRendererProvider);
    final coverDir = ref.watch(coverDirProvider);
    final player = ref.read(playbackControllerProvider.notifier);
    final queueController = ref.read(queueControllerProvider.notifier);
    final isPlaying =
        playback.playerState == PlayerState.playing ||
        playback.playerState == PlayerState.buffering;
    final duration =
        playback.trackInfo?.durationMs?.toInt() ??
        queue.currentItem?.durationMs ??
        0;
    final item = queue.currentItem;
    final subtitle =
        playback.pendingItem?.displayTitle ??
        [
          item?.artist ?? '',
          item?.album ?? '',
        ].where((s) => s.isNotEmpty).join(' · ');
    final progressEnabled =
        item != null && (playback.currentPath?.isNotEmpty ?? false);
    void details() => Navigator.of(context)
        .push(MaterialPageRoute<void>(builder: (_) => const MusicDetailPage()));
    Widget action(IconData icon, String label, VoidCallback onTap) => SizedBox(
      width: 34,
      height: 36,
      child: IconButton(
        onPressed: onTap,
        tooltip: label,
        padding: EdgeInsets.zero,
        icon: Icon(icon, size: 19),
      ),
    );

    return DesktopPlayerBar(
      title: item?.displayTitle ?? l10n.nowPlayingNone,
      subtitle: subtitle.isEmpty ? '让喜欢的音乐，陪伴此刻' : subtitle,
      cover: NowPlayingCover(
        coverDir: coverDir,
        trackId: item?.id,
        cover: item?.cover,
        primaryColor: ArtworkPalette.of(context).accent,
        onTap: item == null ? null : details,
      ),
      isPlaying: isPlaying,
      position: NowPlayingCommon.formatMs(playback.positionMs),
      duration: NowPlayingCommon.formatMs(duration),
      onPlayPause: () => isPlaying ? player.pause() : player.play(),
      onPrevious: () => player.previous(),
      onNext: () => player.next(),
      shuffle: queue.playMode == PlayMode.shuffle,
      repeat:
          queue.playMode == PlayMode.repeatAll ||
          queue.playMode == PlayMode.repeatOne,
      onShuffle: () => queueController.setPlayMode(
        queue.playMode == PlayMode.shuffle
            ? PlayMode.sequential
            : PlayMode.shuffle,
      ),
      onRepeat: () => queueController.setPlayMode(switch (queue.playMode) {
        PlayMode.repeatAll => PlayMode.repeatOne,
        PlayMode.repeatOne => PlayMode.sequential,
        _ => PlayMode.repeatAll,
      }),
      progress: playback.pendingItem != null
          ? const SizedBox(
              height: 6,
              child: Center(child: LinearProgressIndicator(minHeight: 3)),
            )
          : NowPlayingProgressBar(
              durationMs: duration,
              positionMs: playback.positionMs,
              enabled: progressEnabled,
              audioStarted: playback.audioStarted,
              playerState: playback.playerState,
              foregroundColor: ArtworkPalette.of(context).accent,
              onSeekMs: (ms) => player.seekMs(ms),
            ),
      trailing: Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          if (playback.lastError != null)
            action(
              Icons.error_outline,
              playback.lastError!,
              () => ScaffoldMessenger.of(context)
                  .showSnackBar(SnackBar(content: Text(playback.lastError!))),
            ),
          VolumePopupButton(
            volume: playback.desiredVolume,
            iconSize: 19,
            buttonSize: 34,
            enableHover: true,
            onChanged: (v) => player.setVolume(v),
            onToggleMute: () => player.toggleMute(),
          ),
          action(
            Icons.cast_rounded,
            renderer == null ? 'DLNA' : 'DLNA: ${renderer.friendlyName}',
            () async {
              final result = await showDialog<DlnaActionResult>(
                context: context,
                builder: (_) => DlnaDialog(selected: renderer),
              );
              if (result == null) return;
              if (result.applySelection) {
                ref
                    .read(dlnaSelectedRendererProvider.notifier)
                    .set(result.selected);
              }
              if (result.message != null && context.mounted) {
                ScaffoldMessenger.of(context)
                    .showSnackBar(SnackBar(content: Text(result.message!)));
              }
            },
          ),
          action(Icons.lyrics_outlined, '歌词', details),
          if (MediaQuery.sizeOf(context).width >= 1250 &&
              playback.currentPath != null) ...[
            const SizedBox(width: 12),
            AudioFormatBadge(
              path: playback.currentPath!,
              sampleRate: playback.trackInfo?.sampleRate,
            ),
            const SizedBox(width: 6),
          ],
          action(
            Icons.queue_music_rounded,
            l10n.queueTitle,
            () => Navigator.of(
              context,
            ).push(MaterialPageRoute<void>(builder: (_) => const QueuePage())),
          ),
        ],
      ),
    );
  }
}
