import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/ui/pages/library/desktop_library_view.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/ui/widgets/folder_tree.dart';

void main() {
  testWidgets('collection cache refreshes after library results change', (
    tester,
  ) async {
    var section = LibrarySection.albums;
    final results = ValueNotifier<List<TrackLite>>([
      TrackLite(
        id: 1,
        path: 'old.mp3',
        album: 'Old album',
        artist: 'Old artist',
      ),
    ]);
    addTearDown(results.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (_, update) => ValueListenableBuilder<List<TrackLite>>(
              valueListenable: results,
              builder: (_, tracks, _) => DesktopLibraryView(
                tracks: tracks,
                coverDir: '',
                section: section,
                coverBuilder: (_) => const ColoredBox(color: Colors.blue),
                onSectionChanged: (value) => update(() => section = value),
                foldersView: const SizedBox(),
                onAddFolder: () {},
                onScan: (_) {},
                trackListBuilder: (_) => const SizedBox(),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Old album'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('library-tab-artists')));
    await tester.pumpAndSettle();
    results.value = [
      TrackLite(
        id: 2,
        path: 'new.mp3',
        album: 'New album',
        artist: 'New artist',
      ),
    ];
    await tester.pumpAndSettle();
    expect(find.text('Old artist'), findsNothing);
    expect(find.text('New artist'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('library-tab-albums')));
    await tester.pumpAndSettle();
    expect(find.text('Old album'), findsNothing);
    expect(find.text('New album'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  for (final reducedMotion in [false, true]) {
    testWidgets(
      'tabs retain songs and folder state (reduced motion: $reducedMotion)',
      (tester) async {
        var section = LibrarySection.songs;
        final tracks = [
          for (var i = 0; i < 300; i++)
            TrackLite(
              id: i,
              path: '$i.mp3',
              title: 'Song $i',
              album: 'Album $i',
              artist: 'Artist $i',
            ),
        ];
        await tester.pumpWidget(
          MaterialApp(
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            builder: (_, child) => MediaQuery(
              data: MediaQueryData(disableAnimations: reducedMotion),
              child: child!,
            ),
            home: Scaffold(
              body: StatefulBuilder(
                builder: (_, update) => DesktopLibraryView(
                  tracks: tracks,
                  coverDir: '',
                  section: section,
                  coverBuilder: (_) => const ColoredBox(color: Colors.blue),
                  onSectionChanged: (value) => update(() => section = value),
                  onAddFolder: () {},
                  onScan: (_) {},
                  trackListBuilder: (_) => ListView.builder(
                    key: const ValueKey('songs-list'),
                    itemCount: tracks.length,
                    itemExtent: 40,
                    itemBuilder: (_, i) => Text('Song $i'),
                  ),
                  foldersView: FolderTree(
                    roots: const ['D:/Music'],
                    folders: const ['D:/Music', 'D:/Music/Album'],
                    selectedFolder: '',
                    onSelectAll: () {},
                    onSelectFolder: (_) {},
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(
          find.byType(FolderTree, skipOffstage: false),
          findsNothing,
          reason: 'Do not build the folder index before the tab is visited',
        );
        final songs = find.byKey(const ValueKey('songs-list'));
        final scroll = tester.state<ScrollableState>(
          find.descendant(of: songs, matching: find.byType(Scrollable)),
        );
        await tester.drag(songs, const Offset(0, -500));
        await tester.pumpAndSettle();
        final offset = scroll.position.pixels;
        await tester.tap(find.byKey(const ValueKey('library-tab-folders')));
        expect(section, LibrarySection.folders);
        await tester.pump();
        await tester.pumpAndSettle();
        final folderState = tester.state(find.byType(FolderTree));
        expect(find.text('Album'), findsOneWidget);
        await tester.tap(find.byIcon(Icons.expand_less));
        await tester.pumpAndSettle();
        expect(find.text('Album'), findsNothing);
        await tester.tap(find.byKey(const ValueKey('library-tab-songs')));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 60));
        if (!reducedMotion) {
          final tabs = tester
              .widget<TabBarView>(find.byType(TabBarView))
              .controller!;
          expect(tabs.indexIsChanging, isTrue);
        }
        await tester.pumpAndSettle();
        expect(
          identical(
            tester.state<ScrollableState>(
              find.descendant(of: songs, matching: find.byType(Scrollable)),
            ),
            scroll,
          ),
          isTrue,
        );
        expect(scroll.position.pixels, offset);
        // Rapid non-adjacent selection must settle on the final tab.
        for (final name in ['artists', 'albums', 'folders']) {
          await tester.tap(find.byKey(ValueKey('library-tab-$name')));
          await tester.pump();
        }
        await tester.pumpAndSettle();
        expect(
          identical(tester.state(find.byType(FolderTree)), folderState),
          isTrue,
        );
        expect(find.text('Album'), findsNothing);
        expect(tester.takeException(), isNull);
      },
    );
  }

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
