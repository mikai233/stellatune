import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/playlists/logic/playlists_plugin_bridge_service.dart';
import 'package:stellatune/ui/pages/playlists/logic/plugin_playlists_controller.dart';
import 'package:stellatune/ui/pages/playlists/models/playlists_data_models.dart';
import 'package:stellatune/ui/pages/playlists/models/plugin_playlists_state.dart';

void main() {
  for (final failOld in [false, true]) {
    for (final oldFinishesFirst in [false, true]) {
      test(
        'A ${failOld ? 'failure' : 'success'} ${oldFinishesFirst ? 'before' : 'after'} B cannot own B loading',
        () async {
          final h = _Harness();
          final a = h.controller.select(_entry('A'));
          final b = h.controller.select(_entry('B'));
          expect(h.loader.tracks.map((r) => r.entry.key), ['A', 'B']);
          expect(h.selection.entry.key, 'B');
          expect(h.selection.loading, isTrue);
          void finishA() {
            if (failOld) {
              h.loader.tracks[0].result.completeError(StateError('old failed'));
            } else {
              h.loader.tracks[0].result.complete(_page(['old']));
            }
          }

          if (oldFinishesFirst) {
            finishA();
            await a;
            expect(h.selection.loading, isTrue);
            expect(h.selection.error, isNull);
          }
          h.loader.tracks[1].result.complete(_page(['new'], more: true));
          await b;
          if (!oldFinishesFirst) {
            finishA();
            await a;
          }
          expect(h.selection.entry.key, 'B');
          expect(h.titles, ['new']);
          expect(h.selection.loading, isFalse);
          expect(h.selection.error, isNull);

          final more = h.controller.loadMore();
          expect(h.loader.tracks.last.entry.key, 'B');
          h.loader.tracks.last.result.complete(_page(['next']));
          await more;
          expect(h.titles, ['new', 'next']);
        },
      );
    }
  }

  test('A to B to A rejects the first A even though its key matches', () async {
    final h = _Harness();
    final oldA = h.controller.select(_entry('A'));
    final b = h.controller.select(_entry('B'));
    final newA = h.controller.select(_entry('A'));
    h.loader.tracks[0].result.complete(_page(['old A']));
    await oldA;
    expect(h.selection.loading, isTrue);
    h.loader.tracks[2].result.complete(_page(['new A']));
    await newA;
    h.loader.tracks[1].result.completeError(StateError('B failed'));
    await b;
    expect(h.titles, ['new A']);
    expect(h.selection.error, isNull);
  });

  test(
    'returning to a local playlist cancels plugin loading ownership',
    () async {
      final h = _Harness();
      final pending = h.controller.select(_entry('A'));
      h.controller.clearSelection();
      h.loader.tracks.single.result.completeError(StateError('late'));
      await pending;
      expect(h.state.selection, isNull);
      final next = h.controller.select(_entry('B'));
      h.loader.tracks.last.result.complete(_page(['B']));
      await next;
      expect(h.titles, ['B']);
    },
  );

  test(
    'failed first page can retry the same playlist and clear its error',
    () async {
      final h = _Harness();
      final first = h.controller.select(_entry('A'));
      h.loader.tracks.single.result.completeError(StateError('offline'));
      await first;
      expect(h.selection.error, contains('offline'));
      expect(h.selection.loading, isFalse);
      final retry = h.controller.select(_entry('A'));
      expect(h.selection.loading, isTrue);
      expect(h.selection.error, isNull);
      h.loader.tracks.last.result.complete(_page(['loaded']));
      await retry;
      expect(h.titles, ['loaded']);
      expect(h.selection.error, isNull);
    },
  );

  test(
    'paging serializes requests and retries raw offset without duplicates',
    () async {
      final h = _Harness();
      final first = h.controller.select(_entry('A'));
      h.loader.tracks.single.result.complete(
        _page(['one', 'one', 'two'], fetched: 5, more: true),
      );
      await first;
      final snapshot = h.selection.tracks;
      final more = h.controller.loadMore();
      await h.controller.loadMore();
      expect(h.loader.tracks, hasLength(2));
      expect(h.loader.tracks.last.offset, 5);
      h.loader.tracks.last.result.completeError(StateError('page failed'));
      await more;
      expect(h.selection.nextOffset, 5);
      expect(h.selection.loadingMore, isFalse);
      expect(h.titles, ['one', 'two']);

      final retry = h.controller.loadMore();
      expect(h.selection.error, isNull);
      expect(h.loader.tracks.last.offset, 5);
      h.loader.tracks.last.result.complete(_page(['two', 'three'], fetched: 3));
      await retry;
      expect(h.titles, ['one', 'two', 'three']);
      expect(h.selection.nextOffset, 8);
      expect(snapshot.map((track) => track.title), ['one', 'two']);
      expect(() => snapshot.clear(), throwsUnsupportedError);
      await h.controller.loadMore();
      expect(h.loader.tracks, hasLength(3));
    },
  );

  test(
    'old load-more cannot overwrite a cached selection after A to B to A',
    () async {
      final h = _Harness();
      final first = h.controller.select(_entry('A'));
      h.loader.tracks.single.result.complete(_page(['one'], more: true));
      await first;
      final oldMore = h.controller.loadMore();
      final b = h.controller.select(_entry('B'));
      await h.controller.select(_entry('A'));
      expect(h.titles, ['one']);
      expect(h.selection.loadingMore, isFalse);
      final latestMore = h.controller.loadMore();
      h.loader.tracks[3].result.complete(_page(['latest']));
      await latestMore;
      h.loader.tracks[1].result.complete(_page(['old']));
      h.loader.tracks[2].result.complete(_page(['B']));
      await Future.wait([oldMore, b]);
      expect(h.titles, ['one', 'latest']);
    },
  );

  test(
    'small playlist publishes first page while eager paging continues',
    () async {
      final h = _Harness();
      final pending = h.controller.select(_entry('small', count: 3));
      h.loader.tracks.single.result.complete(_page(['one'], more: true));
      await _flush();
      expect(h.titles, ['one']);
      expect(h.selection.loading, isFalse);
      expect(h.selection.loadingMore, isTrue);
      expect(h.loader.tracks.last.offset, 1);
      h.loader.tracks.last.result.complete(_page(['two', 'three']));
      await pending;
      expect(h.titles, ['one', 'two', 'three']);
      expect(h.selection.loadingMore, isFalse);
    },
  );

  test('empty plugin page with hasMore terminates eager loading', () async {
    final h = _Harness();
    final pending = h.controller.select(_entry('unknown', count: null));
    h.loader.tracks.single.result.complete(_page([], more: true));
    await pending;
    expect(h.selection.hasMore, isFalse);
    expect(h.selection.loading, isFalse);
    await h.controller.loadMore();
    expect(h.loader.tracks, hasLength(1));
  });

  test(
    'completed cache revalidation refreshes changed head without stale writes',
    () async {
      final h = _Harness();
      final first = h.controller.select(_entry('A'));
      h.loader.tracks.single.result.complete(_page(['old']));
      await first;
      h.controller.clearSelection();
      await h.controller.select(_entry('A'));
      expect(h.titles, ['old']);
      expect(h.loader.tracks.last.limit, 1);
      h.loader.tracks.last.result.complete(_page(['new']));
      await _flush();
      expect(h.loader.tracks.last.offset, 1);
      h.loader.tracks.last.result.complete(_page([]));
      await _flush();
      expect(h.selection.loading, isTrue);
      expect(h.loader.tracks.last.limit, PluginPlaylistsController.pageSize);
      h.loader.tracks.last.result.complete(_page(['new']));
      await _flush();
      expect(h.titles, ['new']);
    },
  );

  test(
    'stale cache probe stops before tail once another selection owns state',
    () async {
      final h = _Harness();
      final first = h.controller.select(_entry('A'));
      h.loader.tracks.single.result.complete(_page(['old']));
      await first;
      h.controller.clearSelection();
      await h.controller.select(_entry('A'));
      final b = h.controller.select(_entry('B'));
      h.loader.tracks[1].result.complete(_page(['changed']));
      await _flush();
      expect(h.loader.tracks, hasLength(3));
      h.loader.tracks[2].result.complete(_page(['B']));
      await b;
      expect(h.titles, ['B']);
    },
  );

  test(
    'catalog errors stay local and clear on success; removal cancels selection',
    () async {
      final h = _Harness();
      final failed = h.controller.refresh();
      h.loader.catalogs.single.completeError(StateError('catalog offline'));
      await failed;
      expect(h.state.refreshing, isFalse);
      expect(h.state.listError, contains('catalog offline'));
      final selected = h.controller.select(_entry('removed'));
      final refresh = h.controller.refresh();
      expect(h.state.listError, isNull);
      h.loader.catalogs.last.complete(
        PluginPlaylistRefreshResult(entries: [_entry('B')], sourceErrors: []),
      );
      await refresh;
      expect(h.state.selection, isNull);
      h.loader.tracks.single.result.complete(_page(['removed']));
      await selected;
      expect(h.state.selection, isNull);
      expect(h.state.entries.single.key, 'B');
    },
  );

  test('changed catalog count invalidates selected cached pages', () async {
    final h = _Harness();
    final selected = h.controller.select(_entry('A'));
    h.loader.tracks.single.result.complete(_page(['old'], more: true));
    await selected;
    final refresh = h.controller.refresh();
    h.loader.catalogs.single.complete(
      PluginPlaylistRefreshResult(
        entries: [_entry('A', count: 20000)],
        sourceErrors: [],
      ),
    );
    await _flush();
    expect(h.selection.loading, isTrue);
    expect(h.loader.tracks.last.offset, 0);
    h.loader.tracks.last.result.complete(_page(['new']));
    await refresh;
    expect(h.titles, ['new']);
  });

  test('disposing controller discards late failures', () async {
    final loader = _Loader();
    final container = ProviderContainer(
      overrides: [pluginPlaylistLoaderProvider.overrideWithValue(loader.value)],
    );
    container.listen(pluginPlaylistsControllerProvider, (_, _) {});
    final controller = container.read(
      pluginPlaylistsControllerProvider.notifier,
    );
    final pending = controller.select(_entry('A'));
    container.dispose();
    loader.tracks.single.result.completeError(StateError('late failure'));
    await pending;
  });
}

Future<void> _flush() => Future<void>.delayed(Duration.zero);

PluginPlaylistEntry _entry(String key, {int? count = 10001}) =>
    PluginPlaylistEntry(
      key: key,
      pluginId: 'test.plugin',
      pluginName: 'Test',
      typeId: 'catalog',
      typeDisplayName: 'Catalog',
      sourceId: 'source',
      title: key,
      playlistId: key,
      sourceLabel: 'Test',
      trackCount: count,
    );

PluginTrackPage _page(List<String> titles, {int? fetched, bool more = false}) =>
    PluginTrackPage(
      items: [
        for (final title in titles)
          QueueItem(trackId: null, path: title, title: title),
      ],
      fetchedCount: fetched ?? titles.length,
      hasMore: more,
    );

class _TrackRequest {
  _TrackRequest(this.entry, this.offset, this.limit);
  final PluginPlaylistEntry entry;
  final int offset;
  final int limit;
  final result = Completer<PluginTrackPage>();
}

class _Loader {
  final tracks = <_TrackRequest>[];
  final catalogs = <Completer<PluginPlaylistRefreshResult>>[];

  PluginPlaylistLoader get value => (
    fetchPlaylists: () {
      final request = Completer<PluginPlaylistRefreshResult>();
      catalogs.add(request);
      return request.future;
    },
    fetchTracks: (entry, {required offset, required limit}) {
      final request = _TrackRequest(entry, offset, limit);
      tracks.add(request);
      return request.result.future;
    },
  );
}

class _Harness {
  _Harness() {
    container = ProviderContainer(
      overrides: [pluginPlaylistLoaderProvider.overrideWithValue(loader.value)],
    );
    container.listen(pluginPlaylistsControllerProvider, (_, _) {});
    addTearDown(container.dispose);
    controller = container.read(pluginPlaylistsControllerProvider.notifier);
  }
  final loader = _Loader();
  late final ProviderContainer container;
  late final PluginPlaylistsController controller;
  PluginPlaylistsState get state =>
      container.read(pluginPlaylistsControllerProvider);
  PluginPlaylistSelection get selection => state.selection!;
  List<String?> get titles =>
      selection.tracks.map((track) => track.title).toList();
}
