import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/library/catalog_controller.dart';
import 'package:stellatune/library/catalog_track_sort.dart';

LibrarySource source(String id, {List<MediaKind>? kinds}) => LibrarySource(
  id: id,
  name: id,
  local: id == '1',
  available: true,
  browseKinds: kinds ?? MediaKind.values,
  searchKinds: kinds ?? MediaKind.values,
  sorts: CatalogSort.values,
);
CatalogItem item(
  String source,
  String id, {
  MediaKind kind = MediaKind.track,
}) => CatalogItem(
  reference: MediaRef(sourceInstanceId: source, kind: kind, id: id),
  title: id,
  artistRefs: const [],
);

class FakeCatalog extends CatalogBridge {
  final requests = <(CatalogQuery, Completer<CatalogPage>)>[];
  @override
  Future<List<LibrarySource>> sources() async => [source('1'), source('2')];
  @override
  Future<CatalogPage> browse(CatalogQuery query) {
    final c = Completer<CatalogPage>();
    requests.add((query, c));
    return c.future;
  }

  @override
  Future<CatalogItem> detail(MediaRef reference) async =>
      item(reference.sourceInstanceId, reference.id, kind: reference.kind);
}

class LargeCatalog extends FakeCatalog {
  @override
  Future<CatalogPage> browse(CatalogQuery query) async {
    final offset = int.parse(query.cursor ?? '0');
    final end = (offset + query.limit).clamp(0, 615);
    return CatalogPage(
      items: [
        for (var i = offset; i < end; i++)
          item(query.sourceInstanceId, '$i', kind: query.kind),
      ],
      nextCursor: end < 615 ? '$end' : null,
      total: 615,
    );
  }
}

void main() {
  late FakeCatalog bridge;
  late ProviderContainer container;
  late CatalogController controller;
  setUp(() {
    bridge = FakeCatalog();
    container = ProviderContainer(
      overrides: [catalogBridgeProvider.overrideWithValue(bridge)],
    );
    controller = container.read(catalogControllerProvider.notifier);
  });
  tearDown(() => container.dispose());
  Future<void> tick() => Future<void>.delayed(Duration.zero);
  Future<void> start() async {
    final pending = controller.refreshSources();
    await tick();
    bridge.requests.last.$2.complete(CatalogPage(items: [item('1', 'a')]));
    await pending;
  }

  Future<void> completePage(String id, {String? next}) async {
    bridge.requests.last.$2.complete(
      CatalogPage(items: [item('1', id)], nextCursor: next),
    );
    await tick();
  }

  test('column sorting survives later pages and does not refetch or re-sort unchanged data', () async {
    final pending = controller.refreshSources();
    await tick();
    await completePage('z', next: 'page-2');
    controller.sortTracks(CatalogTrackColumn.title);
    bridge.requests.last.$2.complete(CatalogPage(items: [item('1', 'a')]));
    await pending;
    final visible = container.read(catalogVisibleItemsProvider);
    expect(visible.map((r) => r.title), ['a', 'z']);
    expect(container.read(catalogVisibleItemsProvider), same(visible));
    controller.sortTracks(CatalogTrackColumn.title);
    expect(container.read(catalogVisibleItemsProvider).map((r) => r.title), [
      'z',
      'a',
    ]);
    controller.sortTracks(CatalogTrackColumn.original);
    expect(
      container.read(catalogVisibleItemsProvider),
      same(container.read(catalogControllerProvider).items),
    );
    expect(bridge.requests, hasLength(2));
  });

  test(
    'local and remote collections contain all 615 entries in every category',
    () async {
      container.dispose();
      container = ProviderContainer(
        overrides: [catalogBridgeProvider.overrideWithValue(LargeCatalog())],
      );
      controller = container.read(catalogControllerProvider.notifier);
      await controller.refreshSources();
      for (final sourceId in ['1', '2']) {
        await controller.selectSource(sourceId);
        for (final kind in [
          MediaKind.track,
          MediaKind.album,
          MediaKind.artist,
          MediaKind.folder,
        ]) {
          await controller.selectKind(kind);
          final state = container.read(catalogControllerProvider);
          expect(state.items, hasLength(615));
          expect(state.items.last.reference.id, '614');
          expect(state.total, 615);
          expect(state.nextCursor, isNull);
          expect(state.loadingMore, isFalse);
        }
      }
    },
  );

  test(
    'background refresh retains the visible snapshot until all pages arrive',
    () async {
      await start();
      final old = container.read(catalogControllerProvider).items;
      final pending = controller.refresh(preserveItems: true);
      expect(container.read(catalogControllerProvider).items, same(old));
      await completePage('new-a', next: 'next');
      expect(container.read(catalogControllerProvider).items, same(old));
      expect(container.read(catalogControllerProvider).total, 1);
      expect(container.read(catalogControllerProvider).loadingMore, isFalse);
      await completePage('new-b');
      await pending;
      expect(
        container.read(catalogControllerProvider).items.map((r) => r.title),
        ['new-a', 'new-b'],
      );
      expect(container.read(catalogControllerProvider).loading, isFalse);
    },
  );

  test('failed background refresh keeps the old snapshot without mixing continuation pages', () async {
    await start();
    final old = container.read(catalogControllerProvider).items;
    final pending = controller.refresh(preserveItems: true);
    await completePage('replacement', next: 'next');
    bridge.requests.last.$2.completeError(StateError('offline'));
    await pending;
    final failed = container.read(catalogControllerProvider);
    expect(failed.items, same(old));
    expect(failed.nextCursor, isNull);
    expect(failed.error, isNotNull);
    final retry = controller.refresh(preserveItems: true);
    expect(container.read(catalogControllerProvider).items, same(old));
    bridge.requests.last.$2.complete(const CatalogPage(items: []));
    await retry;
    expect(container.read(catalogControllerProvider).items, isEmpty);
    expect(container.read(catalogControllerProvider).total, 0);
    expect(container.read(catalogControllerProvider).error, isNull);
  });

  test(
    'automatically drains all pages without scrolling or a load-more action',
    () async {
      final pending = controller.refreshSources();
      await tick();
      await completePage('a', next: 'second');
      expect(bridge.requests.last.$1.cursor, 'second');
      expect(container.read(catalogControllerProvider).loadingMore, isTrue);
      await completePage('b', next: 'third');
      expect(bridge.requests.last.$1.cursor, 'third');
      await completePage('c');
      await pending;
      final state = container.read(catalogControllerProvider);
      expect(state.items.map((item) => item.title), ['a', 'b', 'c']);
      expect(state.total, 3);
      expect(state.nextCursor, isNull);
      expect(state.loadingMore, isFalse);
    },
  );
  test('source switch discards an outstanding continuation and stops further requests', () async {
    await start();
    final old = controller.refresh();
    await completePage('first', next: 'old-next');
    final stale = bridge.requests.last.$2;
    final next = controller.selectSource('2');
    stale.complete(
      CatalogPage(items: [item('1', 'old')], nextCursor: 'must-not-load'),
    );
    await old;
    expect(container.read(catalogControllerProvider).sourceId, '2');
    expect(container.read(catalogControllerProvider).loading, isTrue);
    bridge.requests.last.$2.complete(CatalogPage(items: [item('2', 'a')]));
    await next;
    expect(
      bridge.requests.any((request) => request.$1.cursor == 'must-not-load'),
      isFalse,
    );
    expect(
      container
          .read(catalogControllerProvider)
          .items
          .single
          .reference
          .sourceInstanceId,
      '2',
    );
  });
  test('failed continuation retains results and retry automatically completes the rest', () async {
    await start();
    final pending = controller.refresh();
    await completePage('a', next: 'opaque-next');
    bridge.requests.last.$2.completeError(StateError('offline'));
    await pending;
    expect(container.read(catalogControllerProvider).items, hasLength(1));
    expect(container.read(catalogControllerProvider).error, isNotNull);
    final retry = controller.retryLoading();
    await controller.retryLoading();
    expect(bridge.requests.last.$1.cursor, 'opaque-next');
    await completePage('b', next: 'last');
    expect(bridge.requests.last.$1.cursor, 'last');
    await completePage('c');
    await retry;
    expect(container.read(catalogControllerProvider).items, hasLength(3));
    expect(container.read(catalogControllerProvider).error, isNull);
  });
  test('detail navigation and back restore a complete collection without refetching', () async {
    await start();
    final open = controller.open(item('1', 'album', kind: MediaKind.album));
    expect(bridge.requests.last.$1.parent?.id, 'album');
    await completePage('inside');
    await open;
    await controller.back();
    expect(bridge.requests, hasLength(2));
    final state = container.read(catalogControllerProvider);
    expect(state.parents, isEmpty);
    expect(state.items.single.reference.id, 'a');
    expect(state.nextCursor, isNull);
  });
  test(
    'search queries the source and automatically loads every matching page',
    () async {
      await start();
      final search = controller.setSearch('unloaded album');
      expect(bridge.requests.last.$1.search, 'unloaded album');
      await completePage('result', next: 'search-next');
      expect(bridge.requests.last.$1.search, 'unloaded album');
      expect(bridge.requests.last.$1.cursor, 'search-next');
      await completePage('last-result');
      await search;
      expect(container.read(catalogControllerProvider).items, hasLength(2));
    },
  );
  for (final duplicate in [true, false]) {
    test(
      'invalid continuation stops automatic loading (duplicate: $duplicate)',
      () async {
        await start();
        final pending = controller.refresh();
        await completePage('a', next: 'same');
        await completePage(
          duplicate ? 'a' : 'b',
          next: duplicate ? null : 'same',
        );
        await pending;
        expect(container.read(catalogControllerProvider).error, isNotNull);
        expect(container.read(catalogControllerProvider).items, hasLength(1));
      },
    );
  }
  test('disposing the provider discards an outstanding continuation', () async {
    await start();
    final pending = controller.refresh();
    await completePage('a', next: 'next');
    container.dispose();
    container = ProviderContainer();
    bridge.requests.last.$2.completeError(StateError('late'));
    await pending;
  });
  test(
    'revisiting an interrupted collection loads the complete collection',
    () async {
      await start();
      final old = controller.refresh();
      await completePage('partial', next: 'stale');
      final stale = bridge.requests.last.$2;
      final next = controller.selectSource('2');
      bridge.requests.last.$2.complete(const CatalogPage(items: []));
      await next;
      final restored = controller.selectSource('1');
      expect(bridge.requests.last.$1.cursor, isNull);
      await completePage('fresh');
      await restored;
      stale.complete(CatalogPage(items: [item('1', 'late')]));
      await old;
      expect(
        container.read(catalogControllerProvider).items.single.title,
        'fresh',
      );
    },
  );
  test('switching sources preserves nested navigation history', () async {
    await start();
    final artist = controller.open(item('1', 'artist', kind: MediaKind.artist));
    bridge.requests.last.$2.complete(
      CatalogPage(items: [item('1', 'album', kind: MediaKind.album)]),
    );
    await artist;
    final album = controller.open(item('1', 'album', kind: MediaKind.album));
    await completePage('song');
    await album;
    final other = controller.selectSource('2');
    bridge.requests.last.$2.complete(const CatalogPage(items: []));
    await other;
    await controller.selectSource('1');
    await controller.back();
    expect(
      container.read(catalogControllerProvider).parents.single.reference.id,
      'artist',
    );
    expect(container.read(catalogControllerProvider).kind, MediaKind.album);
    await controller.back();
    expect(container.read(catalogControllerProvider).parents, isEmpty);
  });
}
