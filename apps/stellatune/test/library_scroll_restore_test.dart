import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/ui/pages/library/desktop_library_view.dart';

void main() {
  for (final section in [LibrarySection.albums, LibrarySection.artists]) {
    for (final grid in [true, false]) {
      testWidgets(
        '${section.name} restores scroll after details (grid: $grid)',
        (tester) async {
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(
                body: DesktopLibraryView(
                  tracks: [
                    for (var i = 0; i < 100; i++)
                      TrackLite(
                        id: i,
                        path: '$i.flac',
                        title: 'Song $i',
                        album: 'Album $i',
                        artist: 'Artist $i',
                      ),
                  ],
                  coverDir: '',
                  coverBuilder: (_) => const ColoredBox(color: Colors.blue),
                  section: section,
                  onSectionChanged: (_) {},
                  foldersView: const SizedBox(),
                  onAddFolder: () {},
                  onScan: (_) {},
                  trackListBuilder: (tracks) => ListView.builder(
                    key: const ValueKey('collection-detail'),
                    itemCount: 50,
                    itemExtent: 40,
                    itemBuilder: (_, i) => Text('${tracks.first.title} - $i'),
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
          if (!grid) {
            await tester.tap(find.byTooltip('列表视图'));
            await tester.pumpAndSettle();
          }
          final collection = find.byType(grid ? GridView : ListView);
          final scrollable = find.descendant(
            of: collection,
            matching: find.byType(Scrollable),
          );
          await tester.drag(collection, const Offset(0, -800));
          await tester.pumpAndSettle();
          final savedOffset = tester
              .state<ScrollableState>(scrollable)
              .position
              .pixels;
          expect(savedOffset, greaterThan(0));
          final card = find
              .descendant(
                of: collection,
                matching: find.byType(grid ? InkWell : ListTile),
              )
              .hitTestable()
              .first;
          final cardY = tester.getTopLeft(card).dy;
          await tester.tap(card);
          await tester.pumpAndSettle();
          await tester.drag(
            find.byKey(const ValueKey('collection-detail')),
            const Offset(0, -200),
          );
          await tester.pumpAndSettle();
          await tester.tap(find.byTooltip('返回'));
          await tester.pumpAndSettle();
          expect(
            tester.state<ScrollableState>(scrollable).position.pixels,
            closeTo(savedOffset, .01),
          );
          expect(tester.getTopLeft(card).dy, closeTo(cardY, .01));
          expect(tester.takeException(), isNull);
        },
      );
    }
  }
}
