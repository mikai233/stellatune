import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/home/home_view.dart';
import 'package:stellatune/ui/pages/home_page.dart';
import 'package:stellatune/ui/pages/shell/desktop_frame.dart';

class _Playback extends PlaybackController {
  @override
  PlaybackState build() => const PlaybackState.initial();

  void progress(int value) => state = state.copyWith(positionMs: value);
}

class _Queue extends QueueController {
  @override
  QueueState build() => const QueueState.empty();

  void populate() => state = state.copyWith(
    items: [
      QueueItem(trackId: BigInt.one, path: 'new.mp3', title: 'New track'),
    ],
    currentIndex: 0,
  );
}

class _PaintCounter extends CustomPainter {
  _PaintCounter(this.onPaint);
  final VoidCallback onPaint;
  @override
  void paint(Canvas canvas, Size size) => onPaint();
  @override
  bool shouldRepaint(_PaintCounter oldDelegate) => false;
}

void main() {
  testWidgets('desktop home ignores progress but still updates queue cards', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final container = ProviderContainer(
      overrides: [
        playbackControllerProvider.overrideWith(_Playback.new),
        queueControllerProvider.overrideWith(_Queue.new),
        coverDirProvider.overrideWithValue(''),
        homeAllTracksProvider.overrideWith((ref) async => <TrackLite>[]),
      ],
    );
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: Scaffold(body: HomePage(onOpenLibrary: () {})),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final view = tester.widget<HomeView>(find.byType(HomeView));
    final playback =
        container.read(playbackControllerProvider.notifier) as _Playback;
    for (var i = 1; i <= 5; i++) {
      playback.progress(i * 200);
      await tester.pump();
      expect(
        identical(tester.widget(find.byType(HomeView)), view),
        isTrue,
        reason: 'Position-only updates should not reconstruct the home view',
      );
    }
    (container.read(queueControllerProvider.notifier) as _Queue).populate();
    await tester.pumpAndSettle();
    expect(
      tester
          .widget<HomeView>(find.byType(HomeView))
          .data
          .continueListening
          .first
          .title,
      'New track',
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('player bar painting does not repaint desktop page content', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final progress = ValueNotifier<double>(0);
    addTearDown(progress.dispose);
    var paints = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: DesktopFrame(
          selectedIndex: 0,
          onDestinationSelected: (_) {},
          playerBar: ValueListenableBuilder<double>(
            valueListenable: progress,
            builder: (_, value, _) => SizedBox(
              height: 78,
              child: LinearProgressIndicator(value: value),
            ),
          ),
          child: CustomPaint(
            painter: _PaintCounter(() => paints++),
            child: const SizedBox.expand(),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final before = paints;
    expect(before, greaterThan(0));
    for (var i = 1; i <= 5; i++) {
      progress.value = i / 10;
      await tester.pumpAndSettle();
    }
    expect(paints, before);
  });
}
