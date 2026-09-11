import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_track_sort.dart';
import 'package:stellatune/ui/pages/library/catalog_column_layout.dart';
import 'package:stellatune/ui/pages/library/catalog_widgets.dart';

class MemoryColumnStore extends SettingsStore {
  Map<String, double> saved = {};
  int writes = 0;
  @override
  Map<String, double> get catalogColumnWidths => saved;
  @override
  Future<void> setCatalogColumnWidths(Map<String, double> widths) async {
    writes++;
    saved = Map.of(widths);
  }
}

void main() {
  test(
    'responsive column widths and gaps remain continuous across breakpoints',
    () {
      for (final saved in [
        <String, double>{},
        {
          'title': 2.0,
          'artist': 6.0,
          'album': 3.0,
          'original': 80.0,
          'duration': 80.0,
        },
      ]) {
        var previous = CatalogColumnLayout(400, 52, saved);
        for (var width = 401; width <= 750; width++) {
          final next = CatalogColumnLayout(width.toDouble(), 52, saved);
          for (final column in CatalogTrackColumn.values) {
            expect(
              ((next.widths[column] ?? 0) - (previous.widths[column] ?? 0))
                  .abs(),
              lessThan(12),
              reason: '$column at $width',
            );
            expect(
              ((next.gaps[column] ?? 0) - (previous.gaps[column] ?? 0)).abs(),
              lessThan(1),
            );
          }
          final total =
              next.widths.values.fold(0.0, (a, b) => a + b) +
              next.gaps.values.fold(0.0, (a, b) => a + b) +
              next.actionsWidth +
              24;
          expect(total, closeTo(width, .001));
          previous = next;
        }
      }
    },
  );

  testWidgets(
    'collapsing columns fade and clip while headers stay aligned with rows',
    (tester) async {
      tester.view.physicalSize = const Size(750, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        ProviderScope(
          child: MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  CatalogTrackHeader(numberWidth: 52, onSort: (_) {}),
                  CatalogTrackRow(
                    numberWidth: 52,
                    index: 10000,
                    onPlay: () {},
                    actions: const SizedBox(),
                    item: const CatalogItem(
                      reference: MediaRef(
                        sourceInstanceId: '1',
                        kind: MediaKind.track,
                        id: '1',
                      ),
                      title: 'Track',
                      artist: 'Artist',
                      album: 'Album',
                      artistRefs: [],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      final rowState = tester.state(find.byType(CatalogTrackRow));
      for (final width in [
        700.0,
        680.0,
        650.0,
        631.0,
        630.0,
        490.0,
        450.0,
        411.0,
        410.0,
        450.0,
        680.0,
        750.0,
      ]) {
        tester.view.physicalSize = Size(width, 300);
        await tester.pump();
        expect(tester.state(find.byType(CatalogTrackRow)), same(rowState));
        for (final column in ['title', 'artist', 'album', 'duration']) {
          final cells = find.byKey(ValueKey('catalog-column-$column'));
          if (cells.evaluate().isEmpty) continue;
          final header = tester.getRect(cells.first);
          final row = tester.getRect(cells.last);
          expect(row.left, closeTo(header.left, .001));
          expect(row.width, closeTo(header.width, .001));
        }
        if (width == 680) {
          final artist = find
              .byKey(const ValueKey('catalog-column-artist'))
              .first;
          expect(
            find.descendant(of: artist, matching: find.byType(ClipRect)),
            findsOneWidget,
          );
          final opacity = tester
              .widget<Opacity>(
                find
                    .descendant(of: artist, matching: find.byType(Opacity))
                    .first,
              )
              .opacity;
          expect(opacity, closeTo(.5, .001));
        }
        expect(tester.takeException(), isNull);
      }
      final container = ProviderScope.containerOf(
        tester.element(find.byType(CatalogTrackRow)),
      );
      expect(container.read(catalogColumnWidthsProvider), isEmpty);
    },
  );

  test('every visible boundary clamps, preserves total width and adapts to resizing', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final controller = container.read(catalogColumnWidthsProvider.notifier);
    for (final width in [420.0, 600.0, 900.0]) {
      var layout = CatalogColumnLayout(width, 52, const {});
      for (var boundary = 0; boundary < layout.columns.length - 1; boundary++) {
        for (final delta in [-10000.0, 10000.0]) {
          controller.resize(layout, boundary, delta);
          layout = CatalogColumnLayout(
            width,
            52,
            container.read(catalogColumnWidthsProvider),
          );
          for (final c in layout.columns) {
            expect(
              layout.widths[c]!,
              greaterThanOrEqualTo(layout.minimums[c]! - .001),
            );
          }
          final total =
              layout.widths.values.fold(0.0, (a, b) => a + b) +
              layout.gaps.values.fold(0.0, (a, b) => a + b) +
              layout.actionsWidth +
              24;
          expect(total, closeTo(width, .001));
        }
      }
    }
  });

  testWidgets(
    'drag aligns headers and rows, saves on release and double click resets',
    (tester) async {
      tester.view.physicalSize = const Size(900, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final store = MemoryColumnStore();
      var sorted = 0;
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            catalogColumnWidthsProvider.overrideWith(
              () => CatalogColumnWidthsController(store),
            ),
          ],
          child: MaterialApp(
            home: Scaffold(
              body: Column(
                children: [
                  CatalogTrackHeader(numberWidth: 52, onSort: (_) => sorted++),
                  CatalogTrackRow(
                    numberWidth: 52,
                    item: const CatalogItem(
                      reference: MediaRef(
                        sourceInstanceId: '1',
                        kind: MediaKind.track,
                        id: '1',
                      ),
                      title: 'Song',
                      artist: 'Example Artist',
                      album: 'Example Album',
                      artistRefs: [],
                    ),
                    index: 99999,
                    onPlay: () {},
                    actions: const SizedBox(),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final artist = find.text('Example Artist');
      final headerArtist = find.text('Artists');
      final before = tester.getTopLeft(artist).dx;
      expect(tester.getTopLeft(headerArtist).dx, closeTo(before, .01));
      final handle = find.byKey(const ValueKey('catalog-resize-title'));
      final start = tester.getCenter(handle);
      final drag = await tester.startGesture(start);
      await drag.moveTo(start + const Offset(20, 0));
      await tester.pump();
      await drag.moveTo(start + const Offset(80, 0));
      await tester.pump();
      final after = tester.getTopLeft(artist).dx;
      expect(after - before, greaterThan(30));
      expect(tester.getTopLeft(headerArtist).dx, closeTo(after, .01));
      expect(store.writes, 0);
      expect(sorted, 0);
      await drag.up();
      await tester.pumpAndSettle();
      expect(store.writes, 1);
      expect(store.saved, isNotEmpty);
      final reopened = ProviderContainer(
        overrides: [
          catalogColumnWidthsProvider.overrideWith(
            () => CatalogColumnWidthsController(store),
          ),
        ],
      );
      expect(reopened.read(catalogColumnWidthsProvider), store.saved);
      reopened.dispose();
      await tester.tap(handle);
      await tester.pump(const Duration(milliseconds: 80));
      await tester.tap(handle);
      await tester.pumpAndSettle();
      expect(tester.getTopLeft(artist).dx, closeTo(before, .01));
      expect(store.saved, isEmpty);
      expect(sorted, 0);
      // The number/cover separation remains intact throughout resizing.
      expect(
        tester.getTopLeft(find.byType(CatalogArtwork)).dx -
            tester.getTopRight(find.text('100000')).dx,
        greaterThanOrEqualTo(12),
      );
      expect(tester.takeException(), isNull);
    },
  );
}
