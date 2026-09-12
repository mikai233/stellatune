import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';

import '../detail_layout_controller.dart';
import 'layouts.dart';

/// Artwork and static metadata subscribe independently from progress/lyrics.
class DetailTrackContent extends ConsumerWidget {
  const DetailTrackContent({
    super.key,
    required this.foregroundColor,
    required this.mobile,
  });

  final Color foregroundColor;
  final bool mobile;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final track = ref.watch(
      queueControllerProvider.select((queue) {
        final item = queue.currentItem;
        return (
          key: item?.stableTrackKey ?? '',
          id: item?.coverId,
          title: item?.displayTitle,
          artist: item?.artist?.trim() ?? '',
          album: item?.album?.trim() ?? '',
          coverKind: item?.cover?.kind,
          coverValue: item?.cover?.value,
          coverMime: item?.cover?.mime,
          catalogItem: item?.catalogItem,
        );
      }),
    );
    final format = ref.watch(
      playbackControllerProvider.select(
        (playback) => (
          path: playback.currentPath,
          sampleRate: playback.trackInfo?.sampleRate,
        ),
      ),
    );
    final coverDir = ref.watch(coverDirProvider);
    final layout = ref.watch(detailLayoutControllerProvider);
    final cover = track.coverKind == null || track.coverValue == null
        ? null
        : QueueCover(
            kind: track.coverKind!,
            value: track.coverValue!,
            mime: track.coverMime,
          );
    final title = track.title ?? l10n.nowPlayingNone;
    final subtitle = [
      track.artist,
      track.album,
    ].where((s) => s.isNotEmpty).join(' - ');
    return LayoutBuilder(
      builder: (context, constraints) {
        if (!mobile && constraints.maxWidth > 700) {
          return WideLayout(
            coverDir: coverDir,
            trackId: track.id,
            trackIdentityKey: track.key,
            cover: cover,
            title: title,
            subtitle: subtitle,
            slideDirection: layout.slideDirection,
            foregroundColor: foregroundColor,
            currentPath: format.path,
            sampleRate: format.sampleRate,
            catalogItem: track.catalogItem,
            maxWidth: constraints.maxWidth,
            maxHeight: constraints.maxHeight,
            hasLyrics: layout.hasLyrics,
          );
        }
        return NarrowLayout(
          coverDir: coverDir,
          trackId: track.id,
          trackIdentityKey: track.key,
          cover: cover,
          title: title,
          subtitle: subtitle,
          slideDirection: layout.slideDirection,
          foregroundColor: foregroundColor,
          currentPath: format.path,
          sampleRate: format.sampleRate,
          catalogItem: track.catalogItem,
          maxHeight: constraints.maxHeight,
          hasLyrics: layout.hasLyrics,
        );
      },
    );
  }
}
