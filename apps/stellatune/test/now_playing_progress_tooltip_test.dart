import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/bridge/bridge.dart' show PlayerState;
import 'package:stellatune/ui/widgets/now_playing_common/now_playing_progress_bar.dart';

void main() {
  testWidgets('hover previews pointer time and dragging keeps tooltip aligned', (
    tester,
  ) async {
    final seeks = <int>[];
    final position = ValueNotifier<int>(12000);
    addTearDown(position.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Align(
            alignment: Alignment.bottomCenter,
            child: SizedBox(
              width: 600,
              height: 78,
              child: Stack(
                children: [
                  Positioned(
                    top: 0,
                    left: 0,
                    right: 0,
                    child: RepaintBoundary(
                      child: ValueListenableBuilder<int>(
                        valueListenable: position,
                        builder: (_, value, _) => NowPlayingProgressBar(
                          minHitHeight: 24,
                          durationMs: 120000,
                          positionMs: value,
                          enabled: true,
                          audioStarted: false,
                          playerState: PlayerState.paused,
                          onSeekMs: seeks.add,
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final bar = tester.getRect(find.byType(NowPlayingProgressBar));
    // Hover and start dragging well below the thin visible track, including
    // before hover has expanded its visual thickness.
    Offset point(double fraction) =>
        Offset(bar.left + bar.width * fraction, bar.top + 18);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);

    for (final entry in {0.25: '00:30', 0.75: '01:30'}.entries) {
      await mouse.moveTo(point(entry.key));
      await tester.pumpAndSettle();
      final tip = find.text('${entry.value} / 02:00');
      expect(tip, findsOneWidget);
      expect(tester.getCenter(tip).dx, closeTo(point(entry.key).dx, 1));
      expect(tester.getRect(tip).bottom, lessThan(bar.top));
    }
    expect(seeks, isEmpty, reason: 'Hover must not seek or move playback');

    // Playback updates must not pull the preview away from a stationary mouse.
    for (final value in [30000, 45000]) {
      position.value = value;
      await tester.pumpAndSettle();
      final tip = find.text('01:30 / 02:00');
      expect(tip, findsOneWidget);
      expect(tester.getCenter(tip).dx, closeTo(point(.75).dx, 1));
    }

    await mouse.down(point(.75));
    await mouse.moveTo(point(.5));
    await tester.pumpAndSettle();
    expect(find.text('01:00 / 02:00'), findsOneWidget);
    expect(
      tester.getCenter(find.text('01:00 / 02:00')).dx,
      closeTo(point(.5).dx, 1),
    );
    await mouse.up();
    await tester.pumpAndSettle();
    expect(seeks.last, 60000);

    await mouse.moveTo(point(.001));
    await tester.pumpAndSettle();
    expect(
      tester.getRect(find.text('00:00 / 02:00')).left,
      greaterThanOrEqualTo(bar.left),
    );
    await mouse.moveTo(point(.999));
    await tester.pumpAndSettle();
    expect(
      tester.getRect(find.text('01:59 / 02:00')).right,
      lessThanOrEqualTo(bar.right),
    );
    await mouse.moveTo(Offset.zero);
    await tester.pumpAndSettle();
    expect(find.text('01:59 / 02:00'), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
