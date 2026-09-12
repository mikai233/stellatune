import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/ui/pages/library/catalog_widgets.dart';

void main() {
  testWidgets('long track list jumps build only the destination viewport', (
    tester,
  ) async {
    final items = List.generate(
      20000,
      (i) => CatalogItem(
        isSegment: false,
        reference: MediaRef(
          sourceInstanceId: 'local',
          kind: MediaKind.track,
          id: '$i',
        ),
        title: 'Track $i',
        artistRefs: const [],
      ),
    );
    var built = 0;
    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          home: Scaffold(
            body: CatalogItemsView(
              locationKey: 'long-library',
              items: items,
              itemExtent: 48,
              itemBuilder: (item, index) {
                built++;
                return CatalogTrackRow(
                  item: item,
                  index: index,
                  onPlay: () {},
                  actions: const SizedBox(),
                );
              },
              footer: const SizedBox(),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final scroll = tester
        .widget<CustomScrollView>(
          find.byKey(const ValueKey('catalog-items-scroll')),
        )
        .controller!;
    final max = scroll.position.maxScrollExtent;
    expect(max, closeTo(items.length * 48 - 600, .1));
    for (final fraction in [.95, .2, 1.0, 0.0]) {
      built = 0;
      scroll.jumpTo(max * fraction);
      await tester.pump();
      expect(built, lessThan(50), reason: 'jump to $fraction');
      expect(scroll.offset, closeTo(max * fraction, .1));
      expect(scroll.position.maxScrollExtent, max);
    }
    scroll.jumpTo(max);
    await tester.pumpAndSettle();
    expect(find.text('Track 19999'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('fast scrolling defers covers and restores them after settling', (
    tester,
  ) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [coverDirProvider.overrideWithValue('missing-test-covers')],
        child: MaterialApp(
          home: Scaffold(
            body: CatalogItemsView(
              locationKey: 'artwork',
              items: List.generate(
                1000,
                (i) => CatalogItem(
                  isSegment: false,
                  reference: MediaRef(
                    sourceInstanceId: 'local',
                    kind: MediaKind.track,
                    id: '$i',
                  ),
                  title: 'Track $i',
                  localTrackId: i,
                  artistRefs: const [],
                ),
              ),
              itemExtent: 48,
              itemBuilder: (item, _) => Align(
                alignment: Alignment.centerLeft,
                child: CatalogArtwork(item: item),
              ),
              footer: const SizedBox(),
            ),
          ),
        ),
      ),
    );
    await tester.pump();
    expect(find.byType(Image), findsWidgets);
    final scroll = tester
        .widget<CustomScrollView>(
          find.byKey(const ValueKey('catalog-items-scroll')),
        )
        .controller!;
    scroll.jumpTo(20000);
    await tester.pump();
    expect(find.byType(Image), findsNothing);
    await tester.pump(const Duration(milliseconds: 180));
    scroll.jumpTo(22000);
    await tester.pump(const Duration(milliseconds: 180));
    expect(find.byType(Image), findsNothing);
    await tester.pump(const Duration(milliseconds: 80));
    expect(find.byType(Image), findsWidgets);
    scroll.jumpTo(0);
    await tester.pump();
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(milliseconds: 300));
    expect(tester.takeException(), isNull);
  });
}
