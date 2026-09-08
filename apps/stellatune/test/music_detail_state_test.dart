import 'dart:async';

import 'package:flutter/material.dart' hide RepeatMode;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/lyrics/lyrics_controller.dart';
import 'package:stellatune/lyrics/lyrics_state.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/music_detail/desktop_music_detail_page.dart';
import 'package:stellatune/ui/pages/music_detail/detail_layout_controller.dart';
import 'package:stellatune/ui/pages/music_detail/mobile_music_detail_page.dart';
import 'package:stellatune/ui/pages/music_detail/music_detail_scaffold.dart';
import 'package:stellatune/ui/pages/music_detail/widgets/bottom_playback_bar.dart';
import 'package:stellatune/ui/pages/music_detail/widgets/cover_image.dart';
import 'package:stellatune/ui/pages/music_detail/widgets/detail_lyrics_panel.dart';
import 'package:stellatune/ui/pages/music_detail/widgets/detail_playback_controls.dart';
import 'package:stellatune/ui/pages/music_detail/widgets/detail_track_content.dart';
import 'package:stellatune/ui/pages/music_detail/widgets/layouts.dart';
import 'package:stellatune/ui/theme/artwork_palette_provider.dart';
import 'package:stellatune/ui/widgets/dynamic_background.dart';

void main() {
  for (final mobile in [false, true]) {
    testWidgets(
      '${mobile ? 'mobile' : 'desktop'} progress and lyric ticks stay below static layout',
      (tester) async {
        final h = _Harness(tester, mobile: mobile);
        await h.mount();
        final counts = <Type, int>{};
        final original = debugOnRebuildDirtyWidget;
        debugOnRebuildDirtyWidget = (element, builtOnce) {
          original?.call(element, builtOnce);
          final type = element.widget.runtimeType;
          counts[type] = (counts[type] ?? 0) + 1;
        };
        addTearDown(() => debugOnRebuildDirtyWidget = original);
        for (var index = 1; index <= 5; index++) {
          h.playback.progress(index * 200);
          h.lyrics.line(index % 2);
          await tester.pump(const Duration(milliseconds: 100));
        }
        for (final type in [
          MusicDetailScaffold,
          DetailTrackContent,
          WideLayout,
          NarrowLayout,
          CoverImage,
        ]) {
          expect(
            counts[type] ?? 0,
            0,
            reason: '$type should ignore position/lyric-line changes',
          );
        }
        expect(counts[DetailPlaybackControls], 5);
        expect(counts[DetailLyricsPanel], 5);
        expect(
          tester
              .widget<BottomPlaybackBar>(find.byType(BottomPlaybackBar))
              .positionMs,
          1000,
        );
        expect(tester.takeException(), isNull);
        await h.hide();
      },
    );

    testWidgets(
      '${mobile ? 'mobile' : 'desktop'} uses shared latest-cover palette',
      (tester) async {
        final first = Completer<DetailArtworkPalette>();
        final second = Completer<DetailArtworkPalette>();
        final h = _Harness(
          tester,
          mobile: mobile,
          loader: (request) =>
              request.trackKey == '1' ? first.future : second.future,
        );
        await h.mount();
        h.queue.select(1);
        await tester.pump(Duration.zero);
        final blue = _palette(Colors.blue);
        second.complete(blue);
        await tester.pump(Duration.zero);
        await tester.pump(Duration.zero);
        expect(
          tester.widget<ShaderBackground>(find.byType(ShaderBackground)).colors,
          blue.colors,
        );
        first.complete(_palette(Colors.red));
        await tester.pump(Duration.zero);
        expect(
          tester.widget<ShaderBackground>(find.byType(ShaderBackground)).colors,
          blue.colors,
        );
        expect(tester.takeException(), isNull);
        await h.hide();
      },
    );
  }

  testWidgets(
    '350ms lyrics transition survives progress ticks and unrelated queue edits',
    (tester) async {
      final h = _Harness(tester);
      await h.mount();
      expect(h.layout.hasLyrics, isTrue);
      h.queue.select(1);
      h.lyrics.empty('2');
      await tester.pump(Duration.zero);
      expect(h.layout.slideDirection, 1);
      expect(h.layout.hasLyrics, isTrue);
      for (var index = 1; index <= 3; index++) {
        h.playback.progress(index * 100);
        if (index == 2) h.queue.renameSource();
        await tester.pump(const Duration(milliseconds: 100));
        expect(h.layout.hasLyrics, isTrue);
      }
      await tester.pump(const Duration(milliseconds: 49));
      expect(h.layout.hasLyrics, isTrue);
      await tester.pump(const Duration(milliseconds: 1));
      expect(h.layout.hasLyrics, isFalse);
      expect(
        tester.widget<WideLayout>(find.byType(WideLayout)).hasLyrics,
        isFalse,
      );
      await h.hide();
    },
  );

  testWidgets('new lyrics supersede a pending layout transition', (
    tester,
  ) async {
    final h = _Harness(tester);
    await h.mount();
    h.queue.select(1);
    h.lyrics.empty('2');
    await tester.pump(Duration.zero);
    await tester.pump(const Duration(milliseconds: 100));
    h.lyrics.ready('2');
    await tester.pump(Duration.zero);
    await tester.pump(const Duration(milliseconds: 300));
    expect(h.layout.hasLyrics, isTrue);
    await h.hide();
  });

  testWidgets('reopening details retries the same failed cover', (
    tester,
  ) async {
    var attempts = 0;
    final green = _palette(Colors.green);
    final h = _Harness(
      tester,
      loader: (_) async {
        attempts++;
        if (attempts == 1) throw StateError('temporarily unavailable');
        return green;
      },
    );
    await h.mount();
    expect(attempts, 1);
    expect(
      tester.widget<ShaderBackground>(find.byType(ShaderBackground)).colors,
      DetailArtworkPalette.neutral.colors,
    );
    await h.hide();
    await tester.pump(Duration.zero);
    await h.mount();
    expect(attempts, 2);
    expect(
      tester.widget<ShaderBackground>(find.byType(ShaderBackground)).colors,
      green.colors,
    );
    await h.hide();
  });
}

DetailArtworkPalette _palette(Color color) => DetailArtworkPalette(
  colors: List<Color>.filled(4, color),
  foreground: Colors.white,
);

class _Playback extends PlaybackController {
  @override
  PlaybackState build() => const PlaybackState.initial();
  void progress(int position) => state = state.copyWith(positionMs: position);
}

class _Queue extends QueueController {
  @override
  QueueState build() => QueueState(
    items: [
      for (var index = 1; index <= 3; index++)
        QueueItem(
          trackId: BigInt.from(index),
          path: '$index.mp3',
          title: 'Track $index',
        ),
    ],
    currentIndex: 0,
    shuffle: false,
    repeatMode: RepeatMode.off,
    order: const [0, 1, 2],
    orderPos: 0,
    source: null,
  );
  void select(int index) =>
      state = state.copyWith(currentIndex: index, orderPos: index);
  void renameSource() => state = state.copyWith(
    source: const QueueSource(type: QueueSourceType.all, label: 'renamed'),
  );
}

class _Lyrics extends LyricsController {
  @override
  LyricsState build() => _ready('1');
  void line(int index) => state = state.copyWith(currentLineIndex: index);
  void empty(String key) => state = state.copyWith(trackKey: key, doc: null);
  void ready(String key) => state = _ready(key);
  LyricsState _ready(String key) => LyricsState(
    enabled: true,
    status: LyricsStatus.ready,
    trackKey: key,
    doc: LyricsDoc(
      trackKey: key,
      source: 'test',
      isSynced: true,
      lines: const [
        LyricLine(text: 'Line one'),
        LyricLine(text: 'Line two'),
      ],
    ),
    currentLineIndex: 0,
    lastError: null,
  );
}

class _Harness {
  _Harness(
    this.tester, {
    this.mobile = false,
    Future<DetailArtworkPalette> Function(ArtworkRequest)? loader,
  }) {
    tester.view.physicalSize = mobile
        ? const Size(440, 950)
        : const Size(1280, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    container = ProviderContainer(
      overrides: [
        playbackControllerProvider.overrideWith(_Playback.new),
        queueControllerProvider.overrideWith(_Queue.new),
        lyricsControllerProvider.overrideWith(_Lyrics.new),
        coverDirProvider.overrideWithValue(''),
        artworkPaletteLoaderProvider.overrideWithValue(
          loader ?? (_) async => DetailArtworkPalette.neutral,
        ),
      ],
    );
    addTearDown(container.dispose);
  }
  final WidgetTester tester;
  final bool mobile;
  late final ProviderContainer container;
  _Playback get playback =>
      container.read(playbackControllerProvider.notifier) as _Playback;
  _Queue get queue =>
      container.read(queueControllerProvider.notifier) as _Queue;
  _Lyrics get lyrics =>
      container.read(lyricsControllerProvider.notifier) as _Lyrics;
  DetailLayoutState get layout =>
      container.read(detailLayoutControllerProvider);

  Widget _app(Widget page) => UncontrolledProviderScope(
    container: container,
    child: MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: page,
    ),
  );

  Future<void> mount() async {
    await tester.pumpWidget(
      _app(
        mobile ? const MobileMusicDetailPage() : const DesktopMusicDetailPage(),
      ),
    );
    await tester.pump(Duration.zero);
    await tester.pump(const Duration(milliseconds: 900));
    await tester.pump(Duration.zero);
  }

  Future<void> hide() async {
    // Keep the application scope alive, as when popping the detail route.
    await tester.pumpWidget(_app(const SizedBox.shrink()));
    await tester.pump(Duration.zero);
  }
}
