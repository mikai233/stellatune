import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'dart:async';

import 'package:stellatune/library/catalog_bridge.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/playlists/logic/playlists_plugin_bridge_service.dart';
import 'package:stellatune/ui/pages/playlists/models/playlists_data_models.dart';
import 'package:stellatune/ui/pages/playlists/models/plugin_playlists_state.dart';

typedef PluginPlaylistLoader = ({
  Future<PluginPlaylistRefreshResult> Function() fetchPlaylists,
  Future<PluginTrackPage> Function(
    PluginPlaylistEntry entry, {
    required int offset,
    String? cursor,
    required int limit,
  })
  fetchTracks,
});

final pluginPlaylistLoaderProvider = Provider<PluginPlaylistLoader>((ref) {
  final bridge = ref.watch(playerBridgeProvider);
  final service = PlaylistsPluginBridgeService(
    ref.watch(catalogBridgeProvider),
  );
  return (
    fetchPlaylists: () => service.fetchPlaylists(bridge: bridge),
    fetchTracks: (entry, {required offset, required limit, cursor}) =>
        service.fetchTrackPage(
          bridge: bridge,
          entry: entry,
          pageSize: limit,
          offset: offset,
          cursor: cursor,
        ),
  );
});

final pluginPlaylistsControllerProvider =
    NotifierProvider.autoDispose<
      PluginPlaylistsController,
      PluginPlaylistsState
    >(PluginPlaylistsController.new);

/// Owns plugin browsing requests. Each selection replaces the previous request
/// generation, including its loading state; no pending playlist can block another.
class PluginPlaylistsController extends Notifier<PluginPlaylistsState> {
  static const pageSize = 200;
  static const eagerLoadThreshold = 10000;

  late PluginPlaylistLoader _loader;
  final _cache = <String, PluginPlaylistSelection>{};
  int _selectionGeneration = 0;
  int _listGeneration = 0;

  @override
  PluginPlaylistsState build() {
    _loader = ref.watch(pluginPlaylistLoaderProvider);
    _selectionGeneration++;
    _listGeneration++;
    _cache.clear();
    return const PluginPlaylistsState();
  }

  Future<void> refresh() async {
    if (state.refreshing) return;
    final generation = ++_listGeneration;
    state = state.copyWith(refreshing: true, listError: null);
    try {
      final result = await _loader.fetchPlaylists();
      if (!ref.mounted || generation != _listGeneration) return;
      final entries = List<PluginPlaylistEntry>.unmodifiable(result.entries);
      final byKey = {for (final entry in entries) entry.key: entry};
      _cache.removeWhere((key, _) => !byKey.containsKey(key));
      state = state.copyWith(
        entries: entries,
        listError: result.aggregatedError,
      );
      if (result.aggregatedError != null) {
        DiagnosticsService.instance.report(
          StateError(result.sourceErrors.join('\n')),
          operation: 'playlists_load',
        );
      }
      final selected = state.selection;
      if (selected != null) {
        final updated = byKey[selected.entry.key];
        if (updated == null) {
          clearSelection();
        } else if (updated.trackCount != selected.entry.trackCount) {
          await select(updated, reload: true);
        } else {
          state = state.copyWith(selection: selected.copyWith(entry: updated));
        }
      }
    } catch (error) {
      if (!ref.mounted || generation != _listGeneration) return;
      state = state.copyWith(
        listError: DiagnosticsService.instance.failureMessage(
          error,
          operation: 'playlist',
        ),
      );
    } finally {
      if (ref.mounted && generation == _listGeneration) {
        state = state.copyWith(refreshing: false);
      }
    }
  }

  void clearSelection() {
    _selectionGeneration++;
    state = state.copyWith(selection: null);
  }

  Future<void> select(PluginPlaylistEntry entry, {bool reload = false}) async {
    final previous = state.selection;
    if (!reload &&
        previous?.entry.key == entry.key &&
        previous?.error == null) {
      return;
    }
    final generation = ++_selectionGeneration;
    final cached = _cache[entry.key];
    if (!reload &&
        cached != null &&
        cached.entry.trackCount == entry.trackCount) {
      state = state.copyWith(selection: cached.copyWith(entry: entry));
      unawaited(_revalidate(entry, cached, generation));
      return;
    }
    _cache.remove(entry.key);
    state = state.copyWith(
      selection: PluginPlaylistSelection(entry: entry, loading: true),
    );
    try {
      var continueEager = false;
      do {
        final selection = state.selection!;
        final page = await _loader.fetchTracks(
          entry,
          offset: selection.nextOffset,
          cursor: selection.nextCursor,
          limit: pageSize,
        );
        if (!_isCurrent(entry.key, generation)) return;
        final loaded = _merge(selection, page);
        continueEager =
            loaded.hasMore &&
            (entry.trackCount == null ||
                entry.trackCount! <= eagerLoadThreshold) &&
            loaded.nextOffset < eagerLoadThreshold;
        _cache[entry.key] = loaded;
        state = state.copyWith(
          selection: loaded.copyWith(loadingMore: continueEager),
        );
      } while (continueEager);
    } catch (error) {
      if (!_isCurrent(entry.key, generation)) return;
      state = state.copyWith(
        selection: state.selection!.copyWith(
          error: DiagnosticsService.instance.failureMessage(
            error,
            operation: 'playlist',
          ),
        ),
      );
    } finally {
      if (_isCurrent(entry.key, generation)) {
        state = state.copyWith(
          selection: state.selection!.copyWith(
            loading: false,
            loadingMore: false,
          ),
        );
      }
    }
  }

  Future<void> loadMore() async {
    final selection = state.selection;
    if (selection == null ||
        selection.loading ||
        selection.loadingMore ||
        !selection.hasMore) {
      return;
    }
    final generation = _selectionGeneration;
    state = state.copyWith(
      selection: selection.copyWith(loadingMore: true, error: null),
    );
    try {
      final page = await _loader.fetchTracks(
        selection.entry,
        offset: selection.nextOffset,
        cursor: selection.nextCursor,
        limit: pageSize,
      );
      if (!_isCurrent(selection.entry.key, generation)) return;
      final loaded = _merge(selection, page);
      _cache[selection.entry.key] = loaded;
      state = state.copyWith(selection: loaded);
    } catch (error) {
      if (!_isCurrent(selection.entry.key, generation)) return;
      state = state.copyWith(
        selection: state.selection!.copyWith(
          error: DiagnosticsService.instance.failureMessage(
            error,
            operation: 'playlist',
          ),
        ),
      );
    } finally {
      if (_isCurrent(selection.entry.key, generation)) {
        state = state.copyWith(
          selection: state.selection!.copyWith(loadingMore: false),
        );
      }
    }
  }

  bool _isCurrent(String key, int generation) =>
      ref.mounted &&
      generation == _selectionGeneration &&
      state.selection?.entry.key == key;

  PluginPlaylistSelection _merge(
    PluginPlaylistSelection previous,
    PluginTrackPage page,
  ) {
    final tracks = [...previous.tracks];
    final seen = {for (final track in tracks) track.stableTrackKey};
    for (final track in page.items) {
      if (seen.add(track.stableTrackKey)) tracks.add(track);
    }
    return previous.copyWith(
      tracks: List<QueueItem>.unmodifiable(tracks),
      // Offsets count raw plugin rows, including duplicates/unplayable entries.
      nextOffset: previous.nextOffset + page.fetchedCount,
      nextCursor: page.nextCursor,
      hasMore: page.hasMore && page.fetchedCount > 0,
      loading: false,
      loadingMore: false,
      error: null,
    );
  }

  Future<void> _revalidate(
    PluginPlaylistEntry entry,
    PluginPlaylistSelection cached,
    int generation,
  ) async {
    if (cached.hasMore) return;
    try {
      final head = await _loader.fetchTracks(entry, offset: 0, limit: 1);
      if (!_isCurrent(entry.key, generation)) return;

      final changed =
          cached.tracks.isEmpty != head.items.isEmpty ||
          (cached.tracks.isNotEmpty &&
              head.items.isNotEmpty &&
              cached.tracks.first.stableTrackKey !=
                  head.items.first.stableTrackKey);
      if (changed) await select(entry, reload: true);
    } catch (_) {
      // Cached data remains usable when this optional freshness check fails.
    }
  }
}
