import 'dart:async';

import 'package:flutter/material.dart' hide RepeatMode;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_controller.dart';
import 'package:stellatune/library/album_sources.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playback_models.dart';
import 'package:stellatune/ui/pages/library/catalog_library_view.dart';
import 'package:stellatune/ui/pages/library/catalog_widgets.dart';
import 'package:stellatune/ui/pages/library/catalog_folder_view.dart';

class _Library implements LibraryBridge {
  @override
  Stream<LibraryEvent> events() => const Stream.empty();
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Catalog extends CatalogBridge {
  _Catalog({
    this.allowPlayback = false,
    this.secondPage,
    this.localAlbum = false,
  });
  final bool localAlbum;
  final bool allowPlayback;
  final Completer<CatalogPage>? secondPage;
  final playback = _Playback();
  Completer<CatalogPage>? refreshPage;
  final calls = <CatalogQuery>[];
  final detailCalls = <MediaRef>[];
  final collecting = Completer<List<CatalogItem>>();
  final cancelled = <String>[];
  List<CatalogItem>? prepared;
  int collections = 0;
  int preparations = 0;
  CatalogItem item(MediaKind kind, String id) => CatalogItem(
    isSegment: false,
    reference: MediaRef(sourceInstanceId: '7', kind: kind, id: id),
    title: '${kind.name} $id',
    artist: kind == MediaKind.track ? 'Artist $id' : null,
    album: kind == MediaKind.track ? 'Album $id' : null,
    albumRef: kind == MediaKind.track
        ? MediaRef(
            sourceInstanceId: '7',
            kind: MediaKind.album,
            id: 'album/$id',
          )
        : null,
    artistRefs: kind == MediaKind.track
        ? [
            MediaRef(
              sourceInstanceId: '7',
              kind: MediaKind.artist,
              id: 'artist/$id',
            ),
            if (id == '201')
              const MediaRef(
                sourceInstanceId: '7',
                kind: MediaKind.artist,
                id: 'another/artist',
              ),
          ]
        : const [],
  );
  @override
  Future<List<LibrarySource>> sources() async => [
    LibrarySource(
      id: '7',
      name: 'Remote server',
      local: localAlbum,
      available: true,
      browseKinds: [
        MediaKind.track,
        MediaKind.album,
        MediaKind.artist,
        MediaKind.folder,
      ],
      searchKinds: [MediaKind.track, MediaKind.album],
      sorts: [CatalogSort.default_],
    ),
  ];
  @override
  Future<CatalogPage> browse(CatalogQuery query) async {
    calls.add(query);
    if (localAlbum) {
      return CatalogPage(
        items: [
          for (var n = 1; n <= 4; n++)
            CatalogItem(
              reference: MediaRef(
                sourceInstanceId: '7',
                kind: MediaKind.track,
                id: '$n',
              ),
              title: 'Song ${(n - 1) % 2 + 1}',
              artistRefs: const [],
              isSegment: n > 2,
              localPath: n > 2 ? '/album/disc.wav' : '/album/$n.mp3',
              audio: CatalogAudioInfo(
                format: n > 2 ? 'WAV' : 'MP3',
                floatingPoint: false,
                sourceDirectory: '/album',
                cuePath: n > 2 ? '/album/disc.cue' : null,
              ),
            ),
        ],
      );
    }
    if (query.search.isNotEmpty && query.kind == MediaKind.track) {
      return CatalogPage(
        items: [item(MediaKind.track, query.search == 'new' ? '999' : '201')],
      );
    }
    if (query.cursor == null && refreshPage != null) return refreshPage!.future;
    if (query.cursor != null && secondPage != null) return secondPage!.future;
    return CatalogPage(
      items: [
        item(query.kind, query.cursor == null ? '001' : '201'),
        if (allowPlayback && query.cursor == null) item(query.kind, '101'),
      ],
      nextCursor: query.cursor == null ? 'page-two' : null,
    );
  }

  @override
  Future<CatalogItem> detail(MediaRef reference) async {
    detailCalls.add(reference);
    return item(reference.kind, reference.id);
  }

  @override
  Future<List<CatalogItem>> collect(CatalogQuery query, String requestId) {
    collections++;
    return collecting.future;
  }

  @override
  Future<void> cancel(String requestId) async {
    cancelled.add(requestId);
  }

  @override
  Future<List<QueueItem>> prepare(List<CatalogItem> items) async {
    preparations++;
    prepared = items;
    if (allowPlayback) {
      return [
        for (final row in items)
          QueueItem(
            trackId: BigInt.parse(row.reference.id),
            path: '',
            catalogItem: row,
          ),
      ];
    }
    throw StateError('Stop before native playback in widget test');
  }
}

class _Queue extends QueueController {
  @override
  QueueState build() => const QueueState.empty();

  void replace(List<QueueItem> items, int index, QueueSource? source) {
    state = QueueState(
      items: items,
      currentIndex: index,
      shuffle: false,
      repeatMode: RepeatMode.off,
      order: List.generate(items.length, (i) => i),
      orderPos: index,
      source: source,
    );
  }
}

class _Playback extends PlaybackController {
  List<QueueItem> queue = [], appended = [];
  int start = -1;
  int replacements = 0;
  final selections = <int>[];
  @override
  PlaybackState build() => const PlaybackState.initial();
  @override
  Future<void> setQueueAndPlayItems(
    List<QueueItem> items, {
    int startIndex = 0,
    QueueSource? source,
  }) async {
    queue = items;
    start = startIndex;
    replacements++;
    (ref.read(queueControllerProvider.notifier) as _Queue).replace(
      items,
      startIndex,
      source,
    );
  }

  @override
  Future<void> playIndex(int index) async {
    selections.add(index);
    start = index;
    final state = ref.read(queueControllerProvider);
    (ref.read(queueControllerProvider.notifier) as _Queue).replace(
      state.items,
      index,
      state.source,
    );
  }

  @override
  Future<void> enqueueItems(List<QueueItem> items) async =>
      appended.addAll(items);
}

void main() {
  testWidgets('local artist links appear immediately and fit narrow columns', (
    tester,
  ) async {
    final bridge = _Catalog();
    const first = MediaRef(
      sourceInstanceId: '1',
      kind: MediaKind.artist,
      id: '"WOVOP"',
    );
    const second = MediaRef(
      sourceInstanceId: '1',
      kind: MediaKind.artist,
      id: '"洛天依"',
    );
    const track = CatalogItem(
      isSegment: false,
      reference: MediaRef(
        sourceInstanceId: '1',
        kind: MediaKind.track,
        id: '42',
      ),
      title: 'Song',
      artist: 'WOVOP / 洛天依',
      artistRefs: [first, second, first],
    );
    final opened = <MediaRef>[];
    Widget screen(double width) => ProviderScope(
      overrides: [catalogBridgeProvider.overrideWithValue(bridge)],
      child: MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: width,
              child: CatalogArtistLinks(
                item: track,
                local: true,
                onOpen: (reference, _) => opened.add(reference),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpWidget(screen(240));
    expect(find.text('WOVOP'), findsOneWidget);
    expect(find.text('洛天依'), findsOneWidget);
    await tester.tap(find.text('WOVOP'));
    await tester.tap(find.text('洛天依'));
    expect(opened, [first, second]);
    expect(bridge.detailCalls, isEmpty);
    expect(find.byType(AlertDialog), findsNothing);
    for (final width in [64.0, 16.0, 1.0]) {
      await tester.pumpWidget(screen(width));
      expect(tester.takeException(), isNull);
    }
  });

  Future<_Catalog> mount(
    WidgetTester tester, {
    bool disableAnimations = false,
    bool allowPlayback = false,
    bool localAlbum = false,
    Completer<CatalogPage>? secondPage,
  }) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final bridge = _Catalog(
      allowPlayback: allowPlayback,
      localAlbum: localAlbum,
      secondPage: secondPage,
    );
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          catalogBridgeProvider.overrideWithValue(bridge),
          libraryBridgeProvider.overrideWithValue(_Library()),
          if (allowPlayback)
            playbackControllerProvider.overrideWith(() => bridge.playback),
          if (allowPlayback) queueControllerProvider.overrideWith(_Queue.new),
        ],
        child: MaterialApp(
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(disableAnimations: disableAnimations),
            child: child!,
          ),
          home: Scaffold(
            body: CatalogLibraryView(
              onAddFolder: () {},
              onScan: (_) {},
              folderManager: const SizedBox(),
            ),
          ),
        ),
      ),
    );
    if (secondPage == null) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump();
      await tester.pump();
    }
    return bridge;
  }

  testWidgets(
    'selecting another row reuses the queue until its order changes',
    (tester) async {
      final bridge = await mount(tester, allowPlayback: true);
      await tester.tap(find.text('track 001'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('track 201'));
      await tester.pumpAndSettle();
      expect(bridge.preparations, 1);
      expect(bridge.playback.replacements, 1);
      expect(bridge.playback.selections, [2]);
      for (var i = 0; i < 2; i++) {
        await tester.tap(find.byKey(const ValueKey('catalog-sort-title')));
        await tester.pumpAndSettle();
      }
      await tester.tap(find.text('track 201'));
      await tester.pumpAndSettle();
      expect(bridge.preparations, 2);
      expect(bridge.playback.replacements, 2);
      expect(bridge.playback.start, 0);
    },
  );

  testWidgets(
    'album source selection only changes the queue on explicit playback',
    (tester) async {
      final bridge = await mount(tester, allowPlayback: true, localAlbum: true);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(CatalogLibraryView)),
      );
      final controller = container.read(catalogControllerProvider.notifier);
      await controller.open(bridge.item(MediaKind.album, 'album'));
      await tester.pumpAndSettle();
      expect(find.byType(CatalogTrackRow), findsNWidgets(2));
      final initial = container.read(catalogSelectedAlbumSourceProvider)!;
      await tester.tap(find.text('Play all'));
      await tester.pumpAndSettle();
      expect(
        bridge.prepared!.map((i) => i.reference),
        initial.items.map((i) => i.reference),
      );
      expect(bridge.playback.queue.length, 2);
      final other = container
          .read(catalogAlbumSourcesProvider)
          .firstWhere((s) => s.id != initial.id);
      await container
          .read(albumSourcePreferencesProvider.notifier)
          .select(
            container.read(catalogControllerProvider).albumPreferenceKey,
            other.id,
          );
      await tester.pumpAndSettle();
      expect(bridge.playback.replacements, 1);
      expect(
        bridge.playback.queue.map((i) => i.catalogItem!.reference),
        initial.items.map((i) => i.reference),
      );
      await tester.tap(find.text('Play all'));
      await tester.pumpAndSettle();
      expect(
        bridge.prepared!.map((i) => i.reference),
        other.items.map((i) => i.reference),
      );
      expect(bridge.playback.replacements, 2);
      await controller.setSearch('Song 2');
      await tester.pumpAndSettle();
      expect(find.byType(CatalogTrackRow), findsOneWidget);
      expect(bridge.playback.replacements, 2);
      expect(bridge.playback.queue.length, 2);
    },
  );

  testWidgets(
    'search and selecting an existing result retain the playing queue',
    (tester) async {
      final bridge = await mount(tester, allowPlayback: true);
      await tester.tap(find.text('track 001'));
      await tester.pumpAndSettle();
      final original = bridge.playback.queue;
      final container = ProviderScope.containerOf(
        tester.element(find.byType(CatalogLibraryView)),
      );
      final controller = container.read(catalogControllerProvider.notifier);
      await controller.setSearch('match');
      await tester.pumpAndSettle();
      expect(find.text('track 001'), findsNothing);
      expect(bridge.playback.queue, same(original));
      await tester.tap(find.text('track 201'));
      await tester.pumpAndSettle();
      expect(bridge.playback.selections, [2]);
      expect(bridge.playback.replacements, 1);
      expect(bridge.playback.queue, same(original));
      expect(bridge.prepared, hasLength(1));
      await controller.selectKind(MediaKind.album);
      await tester.pumpAndSettle();
      await controller.selectKind(MediaKind.track);
      await tester.pumpAndSettle();
      await controller.setSearch('');
      await tester.pumpAndSettle();
      expect(bridge.playback.queue, same(original));
      expect(bridge.playback.replacements, 1);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'a new search result plays after the current song and retains all others',
    (tester) async {
      final bridge = await mount(tester, allowPlayback: true);
      await tester.tap(find.text('track 101'));
      await tester.pumpAndSettle();
      final container = ProviderScope.containerOf(
        tester.element(find.byType(CatalogLibraryView)),
      );
      final source = container.read(queueControllerProvider).source;
      await container.read(catalogControllerProvider.notifier).setSearch('new');
      await tester.pumpAndSettle();
      await tester.tap(find.text('track 999'));
      await tester.pumpAndSettle();
      expect(
        bridge.playback.queue.map((item) => item.catalogItem!.reference.id),
        ['001', '101', '999', '201'],
      );
      expect(bridge.playback.start, 2);
      expect(container.read(queueControllerProvider).source, same(source));
      expect(bridge.collections, 0);
      expect(bridge.prepared, hasLength(1));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'only explicit play all search results adopts the filtered collection',
    (tester) async {
      final bridge = await mount(tester, allowPlayback: true);
      await tester.tap(find.text('track 001'));
      await tester.pumpAndSettle();
      final container = ProviderScope.containerOf(
        tester.element(find.byType(CatalogLibraryView)),
      );
      await container
          .read(catalogControllerProvider.notifier)
          .setSearch('match');
      await tester.pumpAndSettle();
      await tester.tap(find.text('Play all search results'));
      await tester.pump();
      bridge.collecting.complete([bridge.item(MediaKind.track, '201')]);
      await tester.pumpAndSettle();
      expect(bridge.playback.replacements, 2);
      expect(bridge.playback.queue.single.catalogItem!.reference.id, '201');
      expect(bridge.playback.start, 0);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'playing a search result in an empty queue starts only that song',
    (tester) async {
      final bridge = await mount(tester, allowPlayback: true);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(CatalogLibraryView)),
      );
      await container
          .read(catalogControllerProvider.notifier)
          .setSearch('match');
      await tester.pumpAndSettle();
      await tester.tap(find.text('track 201'));
      await tester.pumpAndSettle();
      expect(bridge.playback.queue.single.catalogItem!.reference.id, '201');
      expect(bridge.playback.start, 0);
      expect(bridge.collections, 0);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'row playback queues the complete sorted view at the clicked index; enqueue only adds one',
    (tester) async {
      final bridge = await mount(tester, allowPlayback: true);
      for (var i = 0; i < 2; i++) {
        await tester.tap(find.byKey(const ValueKey('catalog-sort-title')));
        await tester.pumpAndSettle();
      }
      await tester.tap(find.text('track 101'));
      await tester.pumpAndSettle();
      expect(bridge.playback.queue.map((r) => r.catalogItem!.reference.id), [
        '201',
        '101',
        '001',
      ]);
      expect(bridge.playback.start, 1);
      expect(bridge.collections, 0);
      expect(bridge.calls, hasLength(2));
      final originalQueue = bridge.playback.queue;
      final row = find.ancestor(
        of: find.text('track 201'),
        matching: find.byType(CatalogTrackRow),
      );
      await tester.tap(
        find.descendant(of: row, matching: find.byTooltip('Add to queue')),
      );
      await tester.pumpAndSettle();
      expect(bridge.playback.appended.single.catalogItem!.reference.id, '201');
      expect(bridge.playback.queue, same(originalQueue));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'clicking while pages are loading collects the full queue before playback',
    (tester) async {
      final second = Completer<CatalogPage>();
      final bridge = await mount(
        tester,
        allowPlayback: true,
        secondPage: second,
      );
      await tester.tap(find.text('track 101'));
      await tester.pump();
      expect(bridge.playback.queue, isEmpty);
      expect(bridge.collections, 1);
      bridge.collecting.complete([
        for (final id in ['001', '101', '201', '301'])
          bridge.item(MediaKind.track, id),
      ]);
      await tester.pump();
      await tester.pump();
      expect(bridge.playback.queue, hasLength(4));
      expect(bridge.playback.start, 1);
      second.complete(
        CatalogPage(
          items: [
            bridge.item(MediaKind.track, '201'),
            bridge.item(MediaKind.track, '301'),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'metadata links navigate by reference without playing and return to the folder',
    (tester) async {
      final bridge = await mount(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(CatalogLibraryView)),
      );
      await tester.tap(find.text('Album 001'));
      await tester.pumpAndSettle();
      expect(
        bridge.calls.last.parent,
        bridge.item(MediaKind.track, '001').albumRef,
      );
      expect(bridge.calls.last.kind, MediaKind.track);
      await tester.tap(find.byTooltip('Back'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Artist 001'));
      await tester.pumpAndSettle();
      expect(
        bridge.calls.last.parent,
        bridge.item(MediaKind.track, '001').artistRefs.first,
      );
      expect(bridge.calls.last.kind, MediaKind.album);
      await tester.tap(find.byKey(const ValueKey('catalog-tab-folder')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('folder 001'));
      await tester.pumpAndSettle();
      final folder = container
          .read(catalogControllerProvider)
          .parent!
          .reference;
      await tester.tap(find.text('Album 001'));
      await tester.pumpAndSettle();
      expect(find.byType(CatalogFolderView), findsNothing);
      expect(container.read(catalogControllerProvider).parents, hasLength(1));
      await tester.tap(find.byTooltip('Back'));
      await tester.pumpAndSettle();
      expect(find.byType(CatalogFolderView), findsOneWidget);
      expect(
        container.read(catalogControllerProvider).parent!.reference,
        folder,
      );
      expect(bridge.prepared, isNull);
      expect(bridge.collections, 0);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'returning to the library keeps rows and header geometry while refreshing',
    (tester) async {
      final bridge = _Catalog();
      final container = ProviderContainer(
        overrides: [
          catalogBridgeProvider.overrideWithValue(bridge),
          libraryBridgeProvider.overrideWithValue(_Library()),
        ],
      );
      addTearDown(container.dispose);
      Widget screen(bool library) => UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: library
                ? CatalogLibraryView(
                    onAddFolder: () {},
                    onScan: (_) {},
                    folderManager: const SizedBox(),
                  )
                : const Text('Home'),
          ),
        ),
      );
      await tester.pumpWidget(screen(true));
      await tester.pumpAndSettle();
      final before = tester.getRect(find.text('track 001'));
      final old = container.read(catalogControllerProvider).items;
      bridge.refreshPage = Completer<CatalogPage>();
      await tester.pumpWidget(screen(false));
      await tester.pumpWidget(screen(true));
      await tester.pump();
      expect(container.read(catalogControllerProvider).items, same(old));
      expect(find.text('track 201'), findsOneWidget);
      expect(tester.getRect(find.text('track 001')), before);
      expect(find.byType(LinearProgressIndicator), findsNothing);
      bridge.refreshPage!.complete(
        CatalogPage(
          items: [bridge.item(MediaKind.track, '001')],
          nextCursor: 'page-two',
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('track 201'), findsOneWidget);
      expect(tester.getRect(find.text('track 001')), before);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'each artist link navigates directly without a chooser or playback',
    (tester) async {
      final bridge = await mount(tester);
      expect(find.text('artist artist/201'), findsOneWidget);
      await tester.tap(find.text('artist artist/201'));
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(
        bridge.calls.last.parent,
        bridge.item(MediaKind.track, '201').artistRefs.first,
      );
      await tester.tap(find.byTooltip('Back'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('artist another/artist'));
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(
        bridge.calls.last.parent,
        bridge.item(MediaKind.track, '201').artistRefs.last,
      );
      expect(bridge.calls.last.kind, MediaKind.album);
      expect(bridge.prepared, isNull);
      expect(bridge.collections, 0);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'native album, artist and directory navigation loads typed pages',
    (tester) async {
      final bridge = await mount(tester);
      await tester.tap(find.byKey(const ValueKey('catalog-tab-album')));
      await tester.pumpAndSettle();
      expect(bridge.calls.last.kind, MediaKind.album);
      expect(find.text('Load more'), findsNothing);
      expect(find.text('album 201'), findsOneWidget);
      await tester.tap(find.text('album 201'));
      await tester.pumpAndSettle();
      expect(bridge.calls.last.parent?.id, '201');
      expect(bridge.calls.last.kind, MediaKind.track);
      await tester.tap(find.byTooltip('Back'));
      await tester.pumpAndSettle();
      expect(find.text('album 201'), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('catalog-tab-artist')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('artist 001'));
      await tester.pumpAndSettle();
      expect(bridge.calls.last.parent?.kind, MediaKind.artist);
      await tester.tap(find.byKey(const ValueKey('catalog-tab-folder')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('folder 001'));
      await tester.pumpAndSettle();
      expect(bridge.calls.last.parent?.kind, MediaKind.folder);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('column headers toggle ordering and play all uses that order', (
    tester,
  ) async {
    final bridge = await mount(tester);
    List<String> visible() => tester
        .widgetList<CatalogTrackRow>(find.byType(CatalogTrackRow))
        .map((row) => row.item.reference.id)
        .toList();
    expect(find.byType(PopupMenuButton<CatalogSort>), findsNothing);
    final heading = find.byKey(const ValueKey('catalog-sort-title'));
    await tester.tap(heading);
    await tester.pumpAndSettle();
    expect(visible(), ['001', '201']);
    expect(
      find.descendant(of: heading, matching: find.byIcon(Icons.arrow_upward)),
      findsOneWidget,
    );
    await tester.tap(heading);
    await tester.pumpAndSettle();
    expect(visible(), ['201', '001']);
    expect(
      find.descendant(of: heading, matching: find.byIcon(Icons.arrow_downward)),
      findsOneWidget,
    );
    expect(bridge.calls, hasLength(2));
    await tester.tap(find.text('Play all'));
    await tester.pump();
    bridge.collecting.complete([
      bridge.item(MediaKind.track, '001'),
      bridge.item(MediaKind.track, '201'),
    ]);
    await tester.pumpAndSettle();
    expect(bridge.prepared!.map((item) => item.reference.id), ['201', '001']);
    await tester.tap(find.byKey(const ValueKey('catalog-sort-original')));
    await tester.pumpAndSettle();
    expect(visible(), ['001', '201']);
    expect(tester.takeException(), isNull);
  });
  testWidgets('play-all preparation can be cancelled before queue mutation', (
    tester,
  ) async {
    final bridge = await mount(tester);
    await tester.tap(find.text('Play all'));
    await tester.pump();
    expect(find.text('Cancel preparation'), findsOneWidget);
    await tester.tap(find.text('Cancel preparation'));
    await tester.pump();
    expect(bridge.cancelled, hasLength(1));
    bridge.collecting.complete([]);
    await tester.pumpAndSettle();
    expect(find.text('Play all'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets(
    'reduced motion collapses the tree immediately and retains selection',
    (tester) async {
      await mount(tester, disableAnimations: true);
      await tester.tap(find.byKey(const ValueKey('catalog-tab-folder')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('folder 001'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('catalog-toggle-tree')));
      await tester.pump();
      expect(
        tester.getSize(find.byKey(const ValueKey('catalog-tree-pane'))).width,
        0,
      );
      expect(find.byTooltip('Expand folder tree'), findsOneWidget);
      expect(find.text('track 001'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );
}
