import 'dart:io';

import 'package:flutter/material.dart' hide RepeatMode;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/library/catalog_widgets.dart';
import 'package:stellatune/ui/widgets/now_playing_bar.dart';
import 'package:stellatune/ui/widgets/now_playing_bar/desktop_player_bar.dart';
import 'package:stellatune/ui/widgets/now_playing_common/now_playing_cover.dart';
import 'package:stellatune/ui/widgets/now_playing_common/transition_progress.dart';

QueueItem track(int id) => QueueItem(
  trackId: BigInt.from(id),
  id: id,
  path: '$id.mp3',
  title: 'Track $id',
  artist: 'Artist $id',
  album: 'Album $id',
  durationMs: 120000,
);

class _Playback extends PlaybackController {
  @override
  PlaybackState build() => const PlaybackState.initial();
  void pending(QueueItem? item) => state = state.copyWith(pendingItem: item);
}

class _Queue extends QueueController {
  @override
  QueueState build() => QueueState(
    items: [track(1), track(2)],
    currentIndex: 0,
    shuffle: false,
    repeatMode: RepeatMode.off,
    order: const [0, 1],
    orderPos: 0,
    source: null,
  );
}

void main() {
  testWidgets('player bar uses one mode control with distinct states', (
    tester,
  ) async {
    var taps = 0;
    for (final (mode, icon, label) in [
      (PlayMode.sequential, Icons.arrow_forward_rounded, 'Sequential'),
      (PlayMode.shuffle, Icons.shuffle_rounded, 'Shuffle'),
      (PlayMode.repeatAll, Icons.repeat_rounded, 'Repeat all'),
      (PlayMode.repeatOne, Icons.repeat_one_rounded, 'Repeat one'),
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: DesktopPlayerBar(
              title: 'Song',
              subtitle: 'Artist',
              cover: const SizedBox(),
              isPlaying: false,
              position: '00:00',
              duration: '03:00',
              progress: const SizedBox(),
              trailing: const SizedBox(),
              playMode: mode,
              onPlayMode: () => taps++,
            ),
          ),
        ),
      );
      expect(find.byIcon(icon), findsOneWidget);
      expect(find.byType(IconButton), findsNWidgets(4));
      expect(find.byTooltip(label), findsOneWidget);
      await tester.tap(find.byIcon(icon));
      expect(tester.takeException(), isNull);
    }
    expect(taps, 4);
  });
  testWidgets(
    'pending track updates all metadata and reuses decoded library covers immediately',
    (tester) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync(
        'stellatune_cover_transition_',
      );
      for (var id = 1; id <= 2; id++) {
        File('assets/tray/app_icon.png').copySync('${directory.path}/$id');
      }
      final playback = _Playback();
      final container = ProviderContainer(
        overrides: [
          playbackControllerProvider.overrideWith(() => playback),
          queueControllerProvider.overrideWith(_Queue.new),
          coverDirProvider.overrideWithValue(directory.path),
        ],
      );
      addTearDown(() {
        container.dispose();
        PaintingBinding.instance.imageCache.clear();
        PaintingBinding.instance.imageCache.clearLiveImages();
        directory.deleteSync(recursive: true);
      });
      Future<void> show(Widget child) => tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(body: child),
          ),
        ),
      );
      await show(
        Column(
          children: [
            for (var id = 1; id <= 2; id++)
              CatalogArtwork(
                item: CatalogItem(
                  isSegment: false,
                  reference: MediaRef(
                    sourceInstanceId: 'local',
                    kind: MediaKind.track,
                    id: '$id',
                  ),
                  title: 'Track $id',
                  artistRefs: const [],
                  localTrackId: id,
                ),
              ),
          ],
        ),
      );
      final providers = tester
          .widgetList<Image>(find.byType(Image))
          .map((i) => i.image)
          .toList();
      expect(providers, hasLength(2));
      for (var attempt = 0; attempt < 30; attempt++) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 10)),
        );
        await tester.pump();
        if (tester
                .widgetList<RawImage>(find.byType(RawImage))
                .where((image) => image.image != null)
                .length ==
            2) {
          break;
        }
      }
      expect(
        tester
            .widgetList<RawImage>(find.byType(RawImage))
            .where((image) => image.image != null),
        hasLength(2),
      );
      await show(const NowPlayingBar());
      expect(find.byIcon(Icons.receipt_long_outlined), findsNothing);
      final cover = find.descendant(
        of: find.byType(NowPlayingCover),
        matching: find.byType(RawImage),
      );
      expect(tester.widget<RawImage>(cover).image, isNotNull);
      expect(find.text('Track 1'), findsOneWidget);
      playback.pending(track(2));
      await tester.pump();
      expect(find.text('Track 2'), findsOneWidget);
      expect(find.text('Artist 2 · Album 2'), findsOneWidget);
      expect(find.text('Track 1'), findsNothing);
      expect(
        tester.widget<NowPlayingCover>(find.byType(NowPlayingCover)).trackId,
        2,
      );
      expect(tester.widget<RawImage>(cover).image, isNotNull);
      expect(find.byType(LinearProgressIndicator), findsNothing);
      playback.pending(null);
      await tester.pump();
      expect(find.text('Track 1'), findsOneWidget);
      expect(tester.widget<RawImage>(cover).image, isNotNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'short transitions keep progress mounted without flashing a loader',
    (tester) async {
      Future<void> show(bool busy) => tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: NowPlayingTransitionProgress(
              busy: busy,
              child: const SizedBox(height: 24, child: TextField()),
            ),
          ),
        ),
      );
      await show(false);
      final original = tester.state(find.byType(TextField));
      final size = tester.getSize(find.byType(NowPlayingTransitionProgress));
      await show(true);
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byType(LinearProgressIndicator), findsNothing);
      await show(false);
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byType(LinearProgressIndicator), findsNothing);
      expect(tester.state(find.byType(TextField)), same(original));
      expect(tester.getSize(find.byType(NowPlayingTransitionProgress)), size);
      await show(true);
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      expect(tester.state(find.byType(TextField)), same(original));
      expect(tester.getSize(find.byType(NowPlayingTransitionProgress)), size);
      await show(false);
      expect(find.byType(LinearProgressIndicator), findsNothing);
      await show(true);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(milliseconds: 400));
      expect(tester.takeException(), isNull);
    },
  );
}
