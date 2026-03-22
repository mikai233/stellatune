import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/home/widgets/home_page_widgets.dart';
import 'package:stellatune/ui/pages/music_detail_page.dart';

final homeAllTracksProvider = FutureProvider.autoDispose<List<TrackLite>>((
  ref,
) async {
  final bridge = ref.watch(libraryBridgeProvider);
  final tracks = await bridge.listTracks(
    folder: '',
    recursive: true,
    query: '',
    limit: 200,
  );
  return tracks;
});

class HomePage extends ConsumerWidget {
  const HomePage({super.key, required this.onOpenLibrary});

  final VoidCallback onOpenLibrary;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context)!;
    final desktopWide = MediaQuery.sizeOf(context).width >= 900;
    final listTopPadding = desktopWide ? 84.0 : 24.0;
    final queue = ref.watch(queueControllerProvider);
    final coverDir = ref.watch(coverDirProvider);
    final allTracksAsync = ref.watch(homeAllTracksProvider);

    final recentlyAdded = allTracksAsync.value == null
        ? const <TrackLite>[]
        : _buildRecentlyAdded(allTracksAsync.value!);
    final continueListening = _buildContinueListening(queue, maxItems: 10);

    final fallbackTrack = recentlyAdded.isEmpty ? null : recentlyAdded.first;
    final currentItem = queue.currentItem;
    final heroTitle =
        currentItem?.displayTitle ?? _trackTitle(fallbackTrack, l10n);
    final heroSubtitle = currentItem != null
        ? _subtitle(currentItem.artist, currentItem.album, l10n)
        : _subtitle(fallbackTrack?.artist, fallbackTrack?.album, l10n);
    final heroDurationMs =
        currentItem?.durationMs ??
        (fallbackTrack?.durationMs == null
            ? null
            : fallbackTrack!.durationMs!.toInt());
    final heroTrackId = currentItem?.id ?? fallbackTrack?.id.toInt();

    final hasAnyMusic = (queue.items.isNotEmpty) || recentlyAdded.isNotEmpty;

    return Stack(
      children: [
        Positioned.fill(child: HomeBackdrop(theme: theme)),
        ListView(
          padding: EdgeInsets.fromLTRB(24, listTopPadding, 24, 18),
          children: [
            AnimatedSwitcher(
              duration: const Duration(milliseconds: 320),
              reverseDuration: const Duration(milliseconds: 220),
              switchInCurve: const Cubic(0.20, 0.85, 0.25, 1.0),
              switchOutCurve: const Cubic(0.45, 0.0, 0.75, 0.3),
              layoutBuilder: (currentChild, previousChildren) {
                return Stack(
                  fit: StackFit.passthrough,
                  children: [
                    ...previousChildren,
                    ..._singleOrEmpty(currentChild),
                  ],
                );
              },
              transitionBuilder: (child, animation) {
                final offsetTween = Tween<Offset>(
                  begin: const Offset(0.0, 0.025),
                  end: Offset.zero,
                );
                return FadeTransition(
                  opacity: animation,
                  child: SlideTransition(
                    position: animation.drive(offsetTween),
                    child: child,
                  ),
                );
              },
              child: HomeHeroCard(
                key: ValueKey<String>(
                  'hero-$heroTrackId-$heroDurationMs-$heroTitle',
                ),
                heading: l10n.homeKeepListening,
                title: heroTitle,
                subtitle: heroSubtitle,
                durationMs: heroDurationMs,
                trackId: heroTrackId,
                coverDir: coverDir,
                resumeLabel: l10n.homeResume,
                lyricsLabel: l10n.homeLyrics,
                onResume: hasAnyMusic
                    ? () async {
                        final playbackController = ref.read(
                          playbackControllerProvider.notifier,
                        );
                        final index = queue.currentIndex;
                        if (index != null && queue.items.isNotEmpty) {
                          await playbackController.playIndex(index);
                          return;
                        }
                        if (recentlyAdded.isNotEmpty) {
                          await playbackController.setQueueAndPlayTracks(
                            recentlyAdded,
                            startIndex: 0,
                            source: QueueSource(
                              type: QueueSourceType.all,
                              label: l10n.libraryAllMusic,
                            ),
                          );
                        }
                      }
                    : null,
                onLyrics: () {
                  Navigator.of(context).push(
                    MaterialPageRoute<void>(
                      builder: (_) => const MusicDetailPage(),
                    ),
                  );
                },
              ),
            ),
            const SizedBox(height: 22),
            HomeSectionHeader(
              title: l10n.homeContinueListening,
              moreLabel: l10n.homeMore,
              onMore: onOpenLibrary,
            ),
            const SizedBox(height: 10),
            if (continueListening.isEmpty)
              HomeEmptySectionHint(
                message: allTracksAsync.isLoading
                    ? l10n.homeLoading
                    : l10n.homeQueueHint,
              )
            else
              SizedBox(
                height: 192,
                child: ListView.separated(
                  scrollDirection: Axis.horizontal,
                  clipBehavior: Clip.none,
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  itemCount: continueListening.length,
                  separatorBuilder: (_, _) => const SizedBox(width: 12),
                  itemBuilder: (context, index) {
                    final row = continueListening[index];
                    return HomeSlotSwapTransition(
                      childKey: ValueKey<String>(
                        'continue-${row.index}-${row.item.id}',
                      ),
                      child: HomeContinueListeningSquareCard(
                        size: 178,
                        trackId: row.item.id,
                        coverDir: coverDir,
                        title: row.item.displayTitle,
                        subtitle: _subtitle(
                          row.item.artist,
                          row.item.album,
                          l10n,
                        ),
                        durationMs: row.item.durationMs,
                        onTap: () => ref
                            .read(playbackControllerProvider.notifier)
                            .playIndex(row.index),
                      ),
                    );
                  },
                ),
              ),
            const SizedBox(height: 18),
            HomeSectionHeader(
              title: l10n.homeRecentlyAdded,
              moreLabel: l10n.homeMore,
              onMore: onOpenLibrary,
            ),
            const SizedBox(height: 10),
            if (allTracksAsync.hasError)
              HomeEmptySectionHint(message: l10n.homeLoadTracksFailed)
            else if (recentlyAdded.isEmpty)
              HomeEmptySectionHint(
                message: allTracksAsync.isLoading
                    ? l10n.homeLoading
                    : l10n.homeNoTracksHint,
              )
            else
              SizedBox(
                height: 192,
                child: ListView.separated(
                  scrollDirection: Axis.horizontal,
                  clipBehavior: Clip.none,
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  itemCount: recentlyAdded.length,
                  separatorBuilder: (_, _) => const SizedBox(width: 12),
                  itemBuilder: (context, index) {
                    final track = recentlyAdded[index];
                    return HomeSlotSwapTransition(
                      childKey: ValueKey<String>('recent-${track.id}-$index'),
                      child: HomeContinueListeningSquareCard(
                        size: 178,
                        trackId: track.id.toInt(),
                        coverDir: coverDir,
                        title: _trackTitle(track, l10n),
                        subtitle: _subtitle(track.artist, track.album, l10n),
                        durationMs: track.durationMs?.toInt(),
                        onTap: () => ref
                            .read(playbackControllerProvider.notifier)
                            .setQueueAndPlayTracks(
                              recentlyAdded,
                              startIndex: index,
                              source: QueueSource(
                                type: QueueSourceType.all,
                                label: l10n.libraryAllMusic,
                              ),
                            ),
                      ),
                    );
                  },
                ),
              ),
          ],
        ),
      ],
    );
  }

  List<TrackLite> _buildRecentlyAdded(List<TrackLite> allTracks) {
    final sorted = [...allTracks]
      ..sort((a, b) => b.id.toInt().compareTo(a.id.toInt()));
    return sorted.take(12).toList(growable: false);
  }

  List<_QueueCardItem> _buildContinueListening(
    QueueState queue, {
    required int maxItems,
  }) {
    if (queue.items.isEmpty) return const <_QueueCardItem>[];

    final current = queue.currentIndex ?? 0;
    final items = <_QueueCardItem>[];

    final end = (current + maxItems).clamp(0, queue.items.length);
    for (var i = current; i < end; i++) {
      items.add(_QueueCardItem(index: i, item: queue.items[i]));
    }

    if (items.length >= maxItems) return items;

    for (var i = 0; i < queue.items.length && items.length < maxItems; i++) {
      if (i >= current && i < end) continue;
      items.add(_QueueCardItem(index: i, item: queue.items[i]));
    }

    return items;
  }

  String _trackTitle(TrackLite? track, AppLocalizations l10n) {
    if (track == null) return l10n.homeKeepListening;
    final title = (track.title ?? '').trim();
    if (title.isNotEmpty) return title;
    final path = track.path;
    if (path.isEmpty) return l10n.homeUnknownTrack;
    final segments = path.split(RegExp(r'[\\/]+'));
    return segments.isEmpty ? l10n.homeUnknownTrack : segments.last;
  }

  String _subtitle(String? artist, String? album, AppLocalizations l10n) {
    final text = [
      artist?.trim() ?? '',
      album?.trim() ?? '',
    ].where((v) => v.isNotEmpty).join(' • ');
    return text.isEmpty ? l10n.homeUnknownArtist : text;
  }
}

class _QueueCardItem {
  const _QueueCardItem({required this.index, required this.item});

  final int index;
  final QueueItem item;
}

Iterable<T> _singleOrEmpty<T>(T? value) sync* {
  if (value != null) {
    yield value;
  }
}
