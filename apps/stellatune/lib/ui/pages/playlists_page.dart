import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/providers.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/library_controller.dart';
import 'package:stellatune/l10n/app_localizations.dart';
import 'package:stellatune/player/playback_controller.dart';
import 'package:stellatune/player/playability_messages.dart';
import 'package:stellatune/player/queue_controller.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/widgets/track_list.dart';

class PlaylistsPage extends ConsumerStatefulWidget {
  const PlaylistsPage({super.key, this.useGlobalTopBar = false});

  final bool useGlobalTopBar;

  @override
  ConsumerState<PlaylistsPage> createState() => PlaylistsPageState();
}

class _PluginPlaylistEntry {
  const _PluginPlaylistEntry({
    required this.key,
    required this.pluginId,
    required this.pluginName,
    required this.typeId,
    required this.typeDisplayName,
    required this.sourceId,
    required this.title,
    required this.playlistId,
    required this.sourceLabel,
    required this.config,
    this.trackCount,
    this.cover,
    this.playlistRef,
  });

  final String key;
  final String pluginId;
  final String pluginName;
  final String typeId;
  final String typeDisplayName;
  final String sourceId;
  final String title;
  final String playlistId;
  final String sourceLabel;
  final Map<String, Object?> config;
  final int? trackCount;
  final QueueCover? cover;
  final Object? playlistRef;
}

class _SparseTrackCacheEntry<T> {
  const _SparseTrackCacheEntry({
    required this.items,
    required this.nextOffset,
    required this.hasMore,
    required this.pageSize,
    required this.knownTotalCount,
  });

  final List<T> items;
  final int nextOffset;
  final bool hasMore;
  final int pageSize;
  final int? knownTotalCount;
}

class _SparseTrackPage<T> {
  const _SparseTrackPage({
    required this.items,
    required this.fetchedCount,
    required this.hasMore,
  });

  final List<T> items;
  final int fetchedCount;
  final bool hasMore;
}

abstract class _SparseTrackSource<T> {
  const _SparseTrackSource();

  String get cacheKey;
  int get pageSize;
  int get eagerLoadThreshold;
  int? get knownTotalCount;
  bool get eagerPreferred =>
      knownTotalCount == null || knownTotalCount! <= eagerLoadThreshold;

  Future<_SparseTrackPage<T>> fetchPage({required int offset, int? limit});
}

class _InMemorySparseTrackSource<T> extends _SparseTrackSource<T> {
  const _InMemorySparseTrackSource({
    required this.cacheKey,
    required this.items,
    required this.pageSize,
    required this.eagerLoadThreshold,
  });

  @override
  final String cacheKey;
  final List<T> items;
  @override
  final int pageSize;
  @override
  final int eagerLoadThreshold;

  @override
  int? get knownTotalCount => items.length;

  @override
  Future<_SparseTrackPage<T>> fetchPage({
    required int offset,
    int? limit,
  }) async {
    final pageLimit = (limit ?? pageSize).clamp(1, 1000);
    final start = offset.clamp(0, items.length);
    final end = (start + pageLimit).clamp(0, items.length);
    final slice = start >= end
        ? List<T>.empty(growable: false)
        : items.sublist(start, end);
    final fetchedCount = slice.length;
    return _SparseTrackPage<T>(
      items: slice,
      fetchedCount: fetchedCount,
      hasMore: end < items.length,
    );
  }
}

class _PluginSparseTrackSource extends _SparseTrackSource<QueueItem> {
  const _PluginSparseTrackSource({
    required this.entry,
    required this.pageSize,
    required this.eagerLoadThreshold,
    required this.fetcher,
  });

  final _PluginPlaylistEntry entry;
  @override
  final int pageSize;
  @override
  final int eagerLoadThreshold;
  final Future<_SparseTrackPage<QueueItem>> Function({
    required int offset,
    int? limit,
  })
  fetcher;

  @override
  String get cacheKey => entry.key;

  @override
  int? get knownTotalCount => entry.trackCount;

  @override
  Future<_SparseTrackPage<QueueItem>> fetchPage({
    required int offset,
    int? limit,
  }) {
    return fetcher(offset: offset, limit: limit);
  }
}

class PlaylistsPageState extends ConsumerState<PlaylistsPage> {
  static const int _playabilityProbeMargin = 40;
  static const int _playabilityCacheMaxEntries = 12000;
  static const int _pluginPlaylistPageSize = 500;
  static const int _pluginPlaylistEagerLoadThreshold = 10000;
  static const Duration _pluginRuntimeRefreshDebounce = Duration(
    milliseconds: 250,
  );

  final _searchController = TextEditingController();
  bool _playlistsPanelOpen = false;
  bool _autoSelecting = false;
  StreamSubscription<PluginRuntimeEvent>? _pluginRuntimeSub;
  Timer? _pluginRuntimeRefreshTimer;
  int _playabilityRequestSeq = 0;
  int _viewportStart = 0;
  int _viewportEnd = -1;
  String _resultsKey = '';
  final Map<String, String?> _playabilityCache = <String, String?>{};
  Map<int, String> _blockedReasonByTrackId = const <int, String>{};
  List<_PluginPlaylistEntry> _pluginPlaylists = const <_PluginPlaylistEntry>[];
  String? _selectedPluginPlaylistKey;
  List<QueueItem> _pluginPlaylistTracks = const <QueueItem>[];
  bool _loadingPluginPlaylists = false;
  bool _loadingPluginPlaylistTracks = false;
  bool _loadingPluginPlaylistMore = false;
  int _pluginPlaylistNextOffset = 0;
  bool _pluginPlaylistHasMore = false;
  int _pluginTrackLoadSeq = 0;
  String? _pluginPlaylistError;
  final Map<String, _SparseTrackCacheEntry<QueueItem>>
  _pluginPlaylistTracksCache = <String, _SparseTrackCacheEntry<QueueItem>>{};

  bool get isPlaylistsPanelOpen => _playlistsPanelOpen;

  void togglePlaylistsPanel() {
    setState(() => _playlistsPanelOpen = !_playlistsPanelOpen);
  }

  Future<void> createPlaylistFromTopBar() => _createPlaylist(context);

  @override
  void initState() {
    super.initState();
    _pluginRuntimeSub = ref
        .read(playerBridgeProvider)
        .pluginRuntimeEvents()
        .listen((_) => _schedulePluginRuntimeRefresh());
    unawaited(_refreshPluginPlaylists());
  }

  @override
  void dispose() {
    _pluginRuntimeRefreshTimer?.cancel();
    unawaited(_pluginRuntimeSub?.cancel());
    _searchController.dispose();
    super.dispose();
  }

  void _schedulePluginRuntimeRefresh() {
    _pluginRuntimeRefreshTimer?.cancel();
    _pluginRuntimeRefreshTimer = Timer(_pluginRuntimeRefreshDebounce, () {
      if (!mounted) return;
      _playabilityCache.clear();
      _pluginPlaylistTracksCache.clear();
      final results = ref.read(libraryControllerProvider).results;
      unawaited(_refreshTrackPlayability(results, force: true));
    });
  }

  TrackRef _toLocalTrackRef(TrackLite t) =>
      TrackRef(sourceId: 'local', trackId: t.path, locator: t.path);

  String _trackCacheKey(TrackLite t) => '${t.id}|${t.path}';

  String _buildResultsKey(List<TrackLite> items) {
    if (items.isEmpty) return '';
    final buf = StringBuffer();
    for (final t in items) {
      buf
        ..write(t.id)
        ..write('|')
        ..write(t.path)
        ..write(';');
    }
    return buf.toString();
  }

  void _evictPlayabilityCacheIfNeeded() {
    while (_playabilityCache.length > _playabilityCacheMaxEntries) {
      if (_playabilityCache.isEmpty) return;
      _playabilityCache.remove(_playabilityCache.keys.first);
    }
  }

  bool _sameBlockedReasonMap(Map<int, String> next) {
    final current = _blockedReasonByTrackId;
    if (identical(current, next)) return true;
    if (current.length != next.length) return false;
    for (final entry in current.entries) {
      if (next[entry.key] != entry.value) {
        return false;
      }
    }
    return true;
  }

  void _rebuildBlockedReasonByTrackId(List<TrackLite> items) {
    final l10n = AppLocalizations.of(context);
    if (l10n == null) return;
    final blocked = <int, String>{};
    for (final t in items) {
      final reason = _playabilityCache[_trackCacheKey(t)];
      if (reason == null) continue;
      blocked[t.id.toInt()] = localizePlayabilityReason(l10n, reason);
    }
    if (_sameBlockedReasonMap(blocked)) return;
    setState(() => _blockedReasonByTrackId = blocked);
  }

  void _onViewportRangeChanged(int startIndex, int endIndex) {
    if (_viewportStart == startIndex && _viewportEnd == endIndex) {
      return;
    }
    _viewportStart = startIndex;
    _viewportEnd = endIndex;
    final results = ref.read(libraryControllerProvider).results;
    unawaited(_refreshTrackPlayability(results));
  }

  Future<void> _refreshTrackPlayability(
    List<TrackLite> items, {
    bool force = false,
  }) async {
    final key = _buildResultsKey(items);
    if (_resultsKey != key) {
      _resultsKey = key;
      _viewportStart = 0;
      _viewportEnd = -1;
    }

    if (items.isEmpty) {
      if (!mounted) return;
      setState(() => _blockedReasonByTrackId = const <int, String>{});
      return;
    }

    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _rebuildBlockedReasonByTrackId(items);
    });

    final maxIndex = items.length - 1;
    final initialEnd = (items.length - 1).clamp(0, 19).toInt();
    var probeStart = _viewportEnd >= 0 ? _viewportStart : 0;
    var probeEnd = _viewportEnd >= 0 ? _viewportEnd : initialEnd;
    probeStart = (probeStart - _playabilityProbeMargin)
        .clamp(0, maxIndex)
        .toInt();
    probeEnd = (probeEnd + _playabilityProbeMargin).clamp(0, maxIndex).toInt();
    if (probeEnd < probeStart) {
      probeEnd = probeStart;
    }

    final refs = <TrackRef>[];
    final refKeys = <String>[];
    for (var i = probeStart; i <= probeEnd; i++) {
      final t = items[i];
      final cacheKey = _trackCacheKey(t);
      if (!force && _playabilityCache.containsKey(cacheKey)) {
        continue;
      }
      refs.add(_toLocalTrackRef(t));
      refKeys.add(cacheKey);
    }
    if (refs.isEmpty) {
      return;
    }

    final requestSeq = ++_playabilityRequestSeq;
    List<TrackPlayability> verdicts;
    try {
      verdicts = await ref.read(playerBridgeProvider).canPlayTrackRefs(refs);
    } catch (_) {
      return;
    }
    if (!mounted || requestSeq != _playabilityRequestSeq) return;

    final count = verdicts.length < refKeys.length
        ? verdicts.length
        : refKeys.length;
    for (var i = 0; i < count; i++) {
      final verdict = verdicts[i];
      _playabilityCache[refKeys[i]] = verdict.playable
          ? null
          : verdict.reason?.trim() ?? '';
    }
    _evictPlayabilityCacheIfNeeded();
    _rebuildBlockedReasonByTrackId(items);
  }

  String? _asText(Object? value) {
    if (value == null) return null;
    final text = value.toString().trim();
    if (text.isEmpty) return null;
    return text;
  }

  int? _asInt(Object? value) {
    if (value is int) return value;
    if (value is num) return value.toInt();
    return int.tryParse(value?.toString().trim() ?? '');
  }

  QueueCover? _asCover(Object? value) {
    if (value is! Map) return null;
    final map = value.cast<Object?, Object?>();
    final kindRaw = _asText(map['kind'])?.toLowerCase();
    final v = _asText(map['value']);
    if (kindRaw == null || v == null) return null;
    final kind = switch (kindRaw) {
      'url' => QueueCoverKind.url,
      'file' => QueueCoverKind.file,
      'data' => QueueCoverKind.data,
      _ => null,
    };
    if (kind == null) return null;
    return QueueCover(kind: kind, value: v, mime: _asText(map['mime']));
  }

  Map<String, Object?> _decodeJsonObjectOrEmpty(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return <String, Object?>{};
    try {
      final decoded = jsonDecode(text);
      if (decoded is Map<String, Object?>) return decoded;
      if (decoded is Map) return decoded.cast<String, Object?>();
    } catch (_) {}
    return <String, Object?>{};
  }

  Future<void> _refreshPluginPlaylists() async {
    if (_loadingPluginPlaylists) return;
    setState(() {
      _loadingPluginPlaylists = true;
      _pluginPlaylistError = null;
    });
    try {
      final bridge = ref.read(playerBridgeProvider);
      final settings = ref.read(settingsStoreProvider);
      final sourceTypes = await bridge.sourceListTypes();
      logger.d('plugin playlists refresh: source_types=${sourceTypes.length}');
      final merged = <_PluginPlaylistEntry>[];
      final seen = <String>{};
      final sourceErrors = <String>[];

      for (final source in sourceTypes) {
        final configJson = settings.sourceConfigFor(
          pluginId: source.pluginId,
          typeId: source.typeId,
          defaultValue: source.defaultConfigJson,
        );
        final config = _decodeJsonObjectOrEmpty(configJson);
        final beforeCount = merged.length;
        String raw;
        try {
          logger.d(
            'plugin playlists refresh: request list_playlists plugin=${source.pluginId} type=${source.typeId}',
          );
          raw = await bridge.sourceListItemsJson(
            pluginId: source.pluginId,
            typeId: source.typeId,
            configJson: jsonEncode(config),
            requestJson: jsonEncode(<String, Object?>{
              'action': 'list_playlists',
              'limit': 200,
              'offset': 0,
            }),
          );
        } catch (e, s) {
          final reason =
              'list_playlists failed plugin=${source.pluginId} type=${source.typeId}: $e';
          sourceErrors.add(reason);
          logger.w(reason, error: e, stackTrace: s);
          continue;
        }

        dynamic decoded;
        try {
          decoded = jsonDecode(raw);
        } catch (e, s) {
          final reason =
              'list_playlists decode failed plugin=${source.pluginId} type=${source.typeId}';
          sourceErrors.add(reason);
          logger.w(reason, error: e, stackTrace: s);
          continue;
        }
        if (decoded is! List) {
          final reason =
              'list_playlists unexpected payload plugin=${source.pluginId} type=${source.typeId} payload=${decoded.runtimeType}';
          sourceErrors.add(reason);
          logger.w(reason);
          continue;
        }

        for (final row in decoded) {
          if (row is! Map) continue;
          final map = row.cast<Object?, Object?>();
          final kind = _asText(map['kind'])?.toLowerCase();
          if (kind != null && kind != 'playlist') continue;

          final playlistId =
              _asText(map['playlist_id']) ??
              _asText(map['item_id']) ??
              _asText(map['id']);
          if (playlistId == null || playlistId.isEmpty) continue;

          final title =
              _asText(map['title']) ?? _asText(map['name']) ?? playlistId;
          final sourceId = _asText(map['source_id']) ?? source.typeId;
          final sourceLabel =
              _asText(map['source_label']) ??
              '${source.pluginName} / ${source.displayName}';
          final key = '${source.pluginId}::${source.typeId}::$playlistId';
          if (!seen.add(key)) continue;

          merged.add(
            _PluginPlaylistEntry(
              key: key,
              pluginId: source.pluginId,
              pluginName: source.pluginName,
              typeId: source.typeId,
              typeDisplayName: source.displayName,
              sourceId: sourceId,
              title: title,
              playlistId: playlistId,
              sourceLabel: sourceLabel,
              trackCount: _asInt(map['track_count']),
              cover: _asCover(map['cover']),
              playlistRef: map['playlist_ref'],
              config: config,
            ),
          );
        }

        final added = merged.length - beforeCount;
        logger.d(
          'plugin playlists refresh: plugin=${source.pluginId} type=${source.typeId} total_rows=${decoded.length} added=$added',
        );
      }

      String? aggregatedError;
      if (merged.isEmpty && sourceErrors.isNotEmpty) {
        final preview = sourceErrors.take(3).join(' | ');
        final suffix = sourceErrors.length > 3
            ? ' | ...(${sourceErrors.length - 3} more)'
            : '';
        aggregatedError = '$preview$suffix';
      }
      logger.d(
        'plugin playlists refresh done: playlists=${merged.length} errors=${sourceErrors.length}',
      );
      final validKeys = merged.map((entry) => entry.key).toSet();
      _pluginPlaylistTracksCache.removeWhere(
        (key, _) => !validKeys.contains(key),
      );

      if (!mounted) return;
      setState(() {
        _pluginPlaylists = merged;
        _pluginPlaylistError = aggregatedError;
        if (_selectedPluginPlaylistKey != null &&
            !_pluginPlaylists.any((e) => e.key == _selectedPluginPlaylistKey)) {
          _selectedPluginPlaylistKey = null;
          _pluginPlaylistTracks = const <QueueItem>[];
        }
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _pluginPlaylistError = e.toString());
    } finally {
      if (mounted) {
        setState(() => _loadingPluginPlaylists = false);
      }
    }
  }

  _PluginPlaylistEntry? _selectedPluginPlaylist() {
    final key = _selectedPluginPlaylistKey;
    if (key == null || key.isEmpty) return null;
    for (final item in _pluginPlaylists) {
      if (item.key == key) return item;
    }
    return null;
  }

  _PluginSparseTrackSource _pluginTrackSourceFor(_PluginPlaylistEntry entry) {
    return _PluginSparseTrackSource(
      entry: entry,
      pageSize: _pluginPlaylistPageSize,
      eagerLoadThreshold: _pluginPlaylistEagerLoadThreshold,
      fetcher: ({required int offset, int? limit}) =>
          _fetchPluginTrackPage(entry, offset: offset, limit: limit),
    );
  }

  bool _canContinueEagerLoad(
    _SparseTrackSource<QueueItem> source,
    int fetchedRows,
  ) {
    if (!source.eagerPreferred) return false;
    return fetchedRows < source.eagerLoadThreshold;
  }

  Future<_SparseTrackPage<QueueItem>> _fetchPluginTrackPage(
    _PluginPlaylistEntry entry, {
    required int offset,
    int? limit,
  }) async {
    final bridge = ref.read(playerBridgeProvider);
    final pageLimit = (limit ?? _pluginPlaylistPageSize).clamp(1, 1000);
    final request = <String, Object?>{
      'action': 'playlist_tracks',
      'limit': pageLimit,
      'offset': offset,
    };
    if (entry.playlistRef != null) {
      request['playlist_ref'] = entry.playlistRef;
    } else {
      final idNum = int.tryParse(entry.playlistId);
      request['playlist_id'] = idNum ?? entry.playlistId;
    }

    final raw = await bridge.sourceListItemsJson(
      pluginId: entry.pluginId,
      typeId: entry.typeId,
      configJson: jsonEncode(entry.config),
      requestJson: jsonEncode(request),
    );
    final decoded = jsonDecode(raw);
    final items = _parsePluginQueueItems(decoded, entry);
    final fetchedCount = decoded is List ? decoded.length : 0;
    final hasMore = fetchedCount >= pageLimit;
    return _SparseTrackPage<QueueItem>(
      items: items,
      fetchedCount: fetchedCount,
      hasMore: hasMore,
    );
  }

  void _cachePluginPlaylistTracks(
    _SparseTrackSource<QueueItem> source, {
    required _PluginPlaylistEntry entry,
    required List<QueueItem> items,
    required int nextOffset,
    required bool hasMore,
  }) {
    _pluginPlaylistTracksCache[source.cacheKey] =
        _SparseTrackCacheEntry<QueueItem>(
          items: List<QueueItem>.unmodifiable(items),
          nextOffset: nextOffset,
          hasMore: hasMore,
          pageSize: _pluginPlaylistPageSize,
          knownTotalCount: source.knownTotalCount,
        );
  }

  bool _restorePluginPlaylistTracksFromCache(
    _SparseTrackSource<QueueItem> source, {
    required _PluginPlaylistEntry entry,
  }) {
    final cached = _pluginPlaylistTracksCache[source.cacheKey];
    if (cached == null) return false;
    if (cached.pageSize != source.pageSize) {
      _pluginPlaylistTracksCache.remove(source.cacheKey);
      logger.d(
        'plugin playlist tracks: cache_invalidate page_size plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} cached_page_size=${cached.pageSize} expected=${source.pageSize}',
      );
      return false;
    }
    if (cached.knownTotalCount != null &&
        source.knownTotalCount != null &&
        cached.knownTotalCount != source.knownTotalCount) {
      _pluginPlaylistTracksCache.remove(source.cacheKey);
      logger.d(
        'plugin playlist tracks: cache_invalidate track_count plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} cached_track_count=${cached.knownTotalCount} latest_track_count=${source.knownTotalCount}',
      );
      return false;
    }
    setState(() {
      _pluginPlaylistError = null;
      _loadingPluginPlaylistTracks = false;
      _loadingPluginPlaylistMore = false;
      _pluginPlaylistTracks = cached.items;
      _pluginPlaylistNextOffset = cached.nextOffset;
      _pluginPlaylistHasMore = cached.hasMore;
    });
    logger.d(
      'plugin playlist tracks: cache_hit plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} tracks=${cached.items.length} offset=${cached.nextOffset} has_more=${cached.hasMore}',
    );
    unawaited(
      _revalidatePluginPlaylistCache(source, entry: entry, cached: cached),
    );
    return true;
  }

  Future<void> _revalidatePluginPlaylistCache(
    _SparseTrackSource<QueueItem> source, {
    required _PluginPlaylistEntry entry,
    required _SparseTrackCacheEntry<QueueItem> cached,
  }) async {
    if (cached.hasMore) return;
    final selectedKeyAtStart = _selectedPluginPlaylistKey;
    if (selectedKeyAtStart != entry.key) return;
    final loadSeq = _pluginTrackLoadSeq;
    try {
      final head = await source.fetchPage(offset: 0, limit: 1);
      final tail = await source.fetchPage(offset: cached.nextOffset, limit: 1);
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }

      var stale = false;
      var reason = '';
      if (cached.items.isEmpty && head.items.isNotEmpty) {
        stale = true;
        reason = 'empty_cache_but_remote_has_items';
      } else if (cached.items.isNotEmpty && head.items.isNotEmpty) {
        if (cached.items.first.stableTrackKey !=
            head.items.first.stableTrackKey) {
          stale = true;
          reason = 'first_track_changed';
        }
      }
      if (!stale && tail.fetchedCount > 0) {
        stale = true;
        reason = 'tail_has_new_items';
      }
      if (!stale) return;

      logger.d(
        'plugin playlist tracks: cache_stale plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} reason=$reason cached_tracks=${cached.items.length} cached_offset=${cached.nextOffset}',
      );
      _pluginPlaylistTracksCache.remove(source.cacheKey);
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      await _loadPluginPlaylistTracks(entry);
    } catch (e, s) {
      logger.d(
        'plugin playlist tracks: cache_revalidate_skipped plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} reason=$e',
        error: e,
        stackTrace: s,
      );
    }
  }

  Future<void> _selectPluginPlaylist(_PluginPlaylistEntry entry) async {
    if (_selectedPluginPlaylistKey == entry.key) return;
    setState(() {
      _selectedPluginPlaylistKey = entry.key;
      _pluginPlaylistTracks = const <QueueItem>[];
      _pluginPlaylistError = null;
      _pluginPlaylistNextOffset = 0;
      _pluginPlaylistHasMore = false;
      _loadingPluginPlaylistMore = false;
    });
    await _loadPluginPlaylistTracks(entry);
  }

  Future<void> _loadPluginPlaylistTracks(_PluginPlaylistEntry entry) async {
    if (_loadingPluginPlaylistTracks) return;
    final loadSeq = ++_pluginTrackLoadSeq;
    final source = _pluginTrackSourceFor(entry);
    if (_restorePluginPlaylistTracksFromCache(source, entry: entry)) {
      return;
    }
    setState(() {
      _loadingPluginPlaylistTracks = true;
      _loadingPluginPlaylistMore = false;
      _pluginPlaylistError = null;
      _pluginPlaylistTracks = const <QueueItem>[];
      _pluginPlaylistNextOffset = 0;
      _pluginPlaylistHasMore = false;
    });
    try {
      final preferEager = source.eagerPreferred;
      final merged = <QueueItem>[];
      final seen = <String>{};
      var offset = 0;
      var hasMore = false;
      logger.d(
        'plugin playlist tracks: request plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} track_count=${entry.trackCount} eager=$preferEager',
      );

      final firstPage = await source.fetchPage(offset: offset);
      if (!mounted) return;
      if (_selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      for (final item in firstPage.items) {
        if (seen.add(item.stableTrackKey)) {
          merged.add(item);
        }
      }
      offset += firstPage.fetchedCount;
      hasMore = firstPage.hasMore;
      final continueEager =
          hasMore &&
          firstPage.fetchedCount > 0 &&
          _canContinueEagerLoad(source, offset);
      logger.d(
        'plugin playlist tracks: first_page_loaded plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} fetched=${firstPage.fetchedCount} merged=${merged.length} next_offset=$offset has_more=$hasMore continue_eager=$continueEager',
      );
      setState(() {
        _pluginPlaylistTracks = merged;
        _pluginPlaylistNextOffset = offset;
        _pluginPlaylistHasMore = hasMore;
        _loadingPluginPlaylistTracks = false;
        _loadingPluginPlaylistMore = continueEager;
      });
      _cachePluginPlaylistTracks(
        source,
        entry: entry,
        items: merged,
        nextOffset: offset,
        hasMore: hasMore,
      );

      if (!continueEager) {
        if (hasMore) {
          logger.d(
            'plugin playlist tracks: switch_to_paged plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length} next_offset=$offset threshold=$_pluginPlaylistEagerLoadThreshold',
          );
        } else {
          logger.d(
            'plugin playlist tracks: eager_load_done plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length}',
          );
        }
        return;
      }

      var page = 1;
      while (true) {
        final pageResult = await source.fetchPage(offset: offset);
        if (!mounted) return;
        if (_selectedPluginPlaylistKey != entry.key ||
            loadSeq != _pluginTrackLoadSeq) {
          return;
        }
        for (final item in pageResult.items) {
          if (seen.add(item.stableTrackKey)) {
            merged.add(item);
          }
        }
        offset += pageResult.fetchedCount;
        hasMore = pageResult.hasMore;
        page += 1;

        logger.d(
          'plugin playlist tracks: eager_page_loaded plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} page=$page fetched=${pageResult.fetchedCount} merged=${merged.length} next_offset=$offset has_more=$hasMore',
        );

        final keepEager =
            hasMore &&
            pageResult.fetchedCount > 0 &&
            _canContinueEagerLoad(source, offset);
        setState(() {
          _pluginPlaylistTracks = List<QueueItem>.from(merged);
          _pluginPlaylistNextOffset = offset;
          _pluginPlaylistHasMore = hasMore;
          _loadingPluginPlaylistMore = keepEager;
        });
        _cachePluginPlaylistTracks(
          source,
          entry: entry,
          items: merged,
          nextOffset: offset,
          hasMore: hasMore,
        );
        if (!keepEager) break;
      }

      if (!mounted) return;
      if (_selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      if (hasMore) {
        logger.d(
          'plugin playlist tracks: switch_to_paged plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length} next_offset=$offset threshold=$_pluginPlaylistEagerLoadThreshold',
        );
      } else {
        logger.d(
          'plugin playlist tracks: eager_load_done plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} loaded=${merged.length}',
        );
      }
    } catch (e, s) {
      logger.w(
        'plugin playlist tracks failed plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId}',
        error: e,
        stackTrace: s,
      );
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      setState(() => _pluginPlaylistError = e.toString());
    } finally {
      if (mounted &&
          _selectedPluginPlaylistKey == entry.key &&
          loadSeq == _pluginTrackLoadSeq) {
        setState(() {
          _loadingPluginPlaylistTracks = false;
          _loadingPluginPlaylistMore = false;
        });
      }
    }
  }

  Future<void> _loadMorePluginPlaylistTracks() async {
    final entry = _selectedPluginPlaylist();
    if (entry == null) return;
    if (_loadingPluginPlaylistTracks ||
        _loadingPluginPlaylistMore ||
        !_pluginPlaylistHasMore) {
      return;
    }

    final loadSeq = _pluginTrackLoadSeq;
    final offset = _pluginPlaylistNextOffset;
    final source = _pluginTrackSourceFor(entry);
    setState(() => _loadingPluginPlaylistMore = true);
    try {
      final pageResult = await source.fetchPage(offset: offset);
      final fetchedCount = pageResult.fetchedCount;
      final hasMore = pageResult.hasMore;
      if (!mounted) return;
      if (_selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }

      logger.d(
        'plugin playlist tracks: load_more plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId} offset=$offset merged_add=${pageResult.items.length} fetched=$fetchedCount has_more=$hasMore',
      );
      late final List<QueueItem> merged;
      late final int nextOffset;
      setState(() {
        final seen = <String>{
          for (final t in _pluginPlaylistTracks) t.stableTrackKey,
        };
        merged = List<QueueItem>.from(_pluginPlaylistTracks);
        for (final item in pageResult.items) {
          if (seen.add(item.stableTrackKey)) {
            merged.add(item);
          }
        }
        _pluginPlaylistTracks = merged;
        nextOffset = offset + fetchedCount;
        _pluginPlaylistNextOffset = nextOffset;
        _pluginPlaylistHasMore = hasMore;
      });
      _cachePluginPlaylistTracks(
        source,
        entry: entry,
        items: merged,
        nextOffset: nextOffset,
        hasMore: hasMore,
      );
    } catch (e, s) {
      logger.w(
        'plugin playlist tracks load more failed plugin=${entry.pluginId} type=${entry.typeId} playlist=${entry.playlistId}',
        error: e,
        stackTrace: s,
      );
      if (!mounted ||
          _selectedPluginPlaylistKey != entry.key ||
          loadSeq != _pluginTrackLoadSeq) {
        return;
      }
      setState(() => _pluginPlaylistError = e.toString());
    } finally {
      if (mounted &&
          _selectedPluginPlaylistKey == entry.key &&
          loadSeq == _pluginTrackLoadSeq) {
        setState(() => _loadingPluginPlaylistMore = false);
      }
    }
  }

  List<QueueItem> _parsePluginQueueItems(
    dynamic decoded,
    _PluginPlaylistEntry entry,
  ) {
    final items = <QueueItem>[];
    if (decoded is! List) return items;
    for (final row in decoded) {
      if (row is! Map) continue;
      final map = row.cast<Object?, Object?>();
      final kind = _asText(map['kind'])?.toLowerCase();
      if (kind != null && kind != 'track') continue;

      final trackObj = map['track'];
      if (trackObj is! Map) continue;
      final track = trackObj.cast<String, Object?>();

      final sourceId = _asText(map['source_id']) ?? entry.sourceId;
      final trackId =
          _asText(map['track_id']) ?? _asText(track['song_id']) ?? '';
      if (trackId.isEmpty) continue;
      final extHint = _asText(map['ext_hint']) ?? '';
      final pathHint = _asText(map['path_hint']) ?? '';
      final decoderPluginId = _asText(map['decoder_plugin_id']);
      final decoderTypeId = _asText(map['decoder_type_id']);
      final title = _asText(map['title']) ?? _asText(track['title']);
      final artist = _asText(map['artist']) ?? _asText(track['artist']);
      final album = _asText(map['album']) ?? _asText(track['album']);
      final durationMs =
          _asInt(map['duration_ms']) ?? _asInt(track['duration_ms']);
      final cover = _asCover(map['cover']) ?? _asCover(track['cover']);

      final trackRef = buildPluginSourceTrackRef(
        sourceId: sourceId,
        trackId: trackId,
        pluginId: entry.pluginId,
        typeId: entry.typeId,
        config: entry.config,
        track: track,
        extHint: extHint,
        pathHint: pathHint,
        decoderPluginId: decoderPluginId,
        decoderTypeId: decoderTypeId,
      );
      items.add(
        QueueItem(
          track: trackRef,
          title: title,
          artist: artist,
          album: album,
          durationMs: durationMs,
          cover: cover,
        ),
      );
    }
    return items;
  }

  List<QueueItem> _filteredPluginTracks(String query) {
    final q = query.trim().toLowerCase();
    if (q.isEmpty) return _pluginPlaylistTracks;
    return _pluginPlaylistTracks.where((item) {
      final title = (item.title ?? '').toLowerCase();
      final artist = (item.artist ?? '').toLowerCase();
      final album = (item.album ?? '').toLowerCase();
      return title.contains(q) || artist.contains(q) || album.contains(q);
    }).toList();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final coverDir = ref.watch(coverDirProvider);

    final playlists = ref.watch(
      libraryControllerProvider.select((s) => s.playlists),
    );
    final selectedPlaylistId = ref.watch(
      libraryControllerProvider.select((s) => s.selectedPlaylistId),
    );
    final results = ref.watch(
      libraryControllerProvider.select((s) => s.results),
    );
    // TODO(local-sparse): Keep local list in-memory for now.
    // Switch to true sparse range loading after library events/bridge
    // provide stable offset-based incremental fetch semantics.
    final localSparseSource = _InMemorySparseTrackSource<TrackLite>(
      cacheKey: 'local::$selectedPlaylistId',
      items: results,
      pageSize: _pluginPlaylistPageSize,
      eagerLoadThreshold: _pluginPlaylistEagerLoadThreshold,
    );
    final localTracks = localSparseSource.items;
    unawaited(_refreshTrackPlayability(localTracks));
    final likedTrackIds = ref.watch(
      libraryControllerProvider.select((s) => s.likedTrackIds),
    );
    final queueSourceSnapshot = ref.watch(
      queueControllerProvider.select((s) => s.sourceLabel),
    );
    final selectedPluginPlaylist = _selectedPluginPlaylist();
    if (selectedPluginPlaylist == null) {
      _ensurePlaylistSelected(playlists, selectedPlaylistId);
    }

    PlaylistLite? selectedPlaylist;
    if (selectedPluginPlaylist == null && selectedPlaylistId != null) {
      for (final p in playlists) {
        if (p.id.toInt() == selectedPlaylistId) {
          selectedPlaylist = p;
          break;
        }
      }
    }

    final selectionSourceLabel = selectedPluginPlaylist != null
        ? '${selectedPluginPlaylist.sourceLabel} - ${selectedPluginPlaylist.title}'
        : (selectedPlaylist == null
              ? l10n.queueSourceUnset
              : _playlistDisplayName(l10n, selectedPlaylist));
    final queueSourceLabel = (queueSourceSnapshot ?? '').trim().isEmpty
        ? l10n.queueSourceUnset
        : queueSourceSnapshot!.trim();
    final pluginFilterActive = _searchController.text.trim().isNotEmpty;
    final pluginVisibleTracks = _filteredPluginTracks(_searchController.text);

    return LayoutBuilder(
      builder: (context, constraints) {
        final panelWidth = constraints.maxWidth < 760
            ? (constraints.maxWidth * 0.84).clamp(280.0, 360.0)
            : (constraints.maxWidth * 0.34).clamp(300.0, 380.0);
        final content = Expanded(
          child: ClipRect(
            child: Stack(
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
                  child: selectedPluginPlaylist == null
                      ? _PlaylistTracksPane(
                          searchController: _searchController,
                          queueSourceLabel: queueSourceLabel,
                          selectedLabel: selectedPlaylist == null
                              ? l10n.queueSourceUnset
                              : _playlistDisplayName(l10n, selectedPlaylist),
                          playlists: playlists,
                          selectedPlaylistId: selectedPlaylistId,
                          results: localTracks,
                          likedTrackIds: likedTrackIds,
                          coverDir: coverDir,
                          onSearchChanged: (q) => ref
                              .read(libraryControllerProvider.notifier)
                              .setQuery(q),
                          onActivate: (index, items) async {
                            final source = QueueSource(
                              type: QueueSourceType.playlist,
                              playlistId: selectedPlaylistId,
                              label: selectionSourceLabel,
                            );
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .setQueueAndPlayTracks(
                                  items,
                                  startIndex: index,
                                  source: source,
                                );
                          },
                          onEnqueue: (track) async {
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .enqueueTracks([track]);
                          },
                          onSetLiked: (track, liked) async {
                            await ref
                                .read(libraryControllerProvider.notifier)
                                .setTrackLiked(track.id.toInt(), liked);
                          },
                          onAddToPlaylist: (track, playlistId) async {
                            await ref
                                .read(libraryControllerProvider.notifier)
                                .addTrackToPlaylist(
                                  playlistId,
                                  track.id.toInt(),
                                );
                          },
                          onRemoveFromPlaylist: (track, playlistId) async {
                            await ref
                                .read(libraryControllerProvider.notifier)
                                .removeTrackFromPlaylist(
                                  playlistId,
                                  track.id.toInt(),
                                );
                          },
                          onMoveInCurrentPlaylist: selectedPlaylistId == null
                              ? null
                              : (track, newIndex) async {
                                  await ref
                                      .read(libraryControllerProvider.notifier)
                                      .moveTrackInPlaylist(
                                        playlistId: selectedPlaylistId,
                                        trackId: track.id.toInt(),
                                        newIndex: newIndex,
                                      );
                                },
                          onBatchAddToPlaylist: (tracks, playlistId) async {
                            await ref
                                .read(libraryControllerProvider.notifier)
                                .addTracksToPlaylist(
                                  playlistId: playlistId,
                                  trackIds: tracks
                                      .map((t) => t.id.toInt())
                                      .toList(),
                                );
                          },
                          onBatchRemoveFromCurrentPlaylist:
                              selectedPlaylistId == null
                              ? null
                              : (tracks, playlistId) async {
                                  await ref
                                      .read(libraryControllerProvider.notifier)
                                      .removeTracksFromPlaylist(
                                        playlistId: playlistId,
                                        trackIds: tracks
                                            .map((t) => t.id.toInt())
                                            .toList(),
                                      );
                                },
                          blockedReasonByTrackId: _blockedReasonByTrackId,
                          onViewportRangeChanged: _onViewportRangeChanged,
                        )
                      : _PluginPlaylistTracksPane(
                          searchController: _searchController,
                          queueSourceLabel: queueSourceLabel,
                          selectedLabel:
                              '${selectedPluginPlaylist.sourceLabel} - ${selectedPluginPlaylist.title}',
                          sourceLabel: selectedPluginPlaylist.sourceLabel,
                          tracks: pluginVisibleTracks,
                          loading: _loadingPluginPlaylistTracks,
                          loadingMore: _loadingPluginPlaylistMore,
                          hasMore: _pluginPlaylistHasMore,
                          filterActive: pluginFilterActive,
                          error: _pluginPlaylistError,
                          onSearchChanged: (_) => setState(() {}),
                          onLoadMore: _loadMorePluginPlaylistTracks,
                          onActivate: (index, items) async {
                            final source = QueueSource(
                              type: QueueSourceType.all,
                              label: selectionSourceLabel,
                            );
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .setQueueAndPlayItems(
                                  items,
                                  startIndex: index,
                                  source: source,
                                );
                          },
                          onEnqueue: (item) async {
                            await ref
                                .read(playbackControllerProvider.notifier)
                                .enqueueItems([item]);
                          },
                          onPlayAll: pluginVisibleTracks.isEmpty
                              ? null
                              : () async {
                                  final source = QueueSource(
                                    type: QueueSourceType.all,
                                    label: selectionSourceLabel,
                                  );
                                  await ref
                                      .read(playbackControllerProvider.notifier)
                                      .setQueueAndPlayItems(
                                        pluginVisibleTracks,
                                        startIndex: 0,
                                        source: source,
                                      );
                                },
                          onEnqueueAll: pluginVisibleTracks.isEmpty
                              ? null
                              : () async {
                                  await ref
                                      .read(playbackControllerProvider.notifier)
                                      .enqueueItems(pluginVisibleTracks);
                                },
                        ),
                ),
                if (_playlistsPanelOpen)
                  Positioned.fill(
                    child: GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: () => setState(() => _playlistsPanelOpen = false),
                      child: const SizedBox.expand(),
                    ),
                  ),
                Align(
                  alignment: Alignment.centerLeft,
                  child: AnimatedSlide(
                    duration: const Duration(milliseconds: 260),
                    curve: Curves.easeOutCubic,
                    offset: _playlistsPanelOpen
                        ? Offset.zero
                        : const Offset(-1.0, 0),
                    child: SizedBox(
                      width: panelWidth,
                      child: _PlaylistsDrawerPanel(
                        playlists: playlists,
                        selectedPlaylistId: selectedPlaylistId,
                        pluginPlaylists: _pluginPlaylists,
                        selectedPluginPlaylistKey: _selectedPluginPlaylistKey,
                        onSelect: (id) {
                          if (_selectedPluginPlaylistKey != null) {
                            setState(() {
                              _selectedPluginPlaylistKey = null;
                              _pluginPlaylistTracks = const <QueueItem>[];
                              _pluginPlaylistError = null;
                            });
                          }
                          ref
                              .read(libraryControllerProvider.notifier)
                              .selectPlaylist(id);
                        },
                        onSelectPlugin: (entry) async {
                          await _selectPluginPlaylist(entry);
                        },
                        onRename: (id, currentName) async {
                          final nextName = await _promptPlaylistName(
                            context,
                            title: l10n.playlistRenameTitle,
                            initialValue: currentName,
                          );
                          if (nextName == null) return;
                          await ref
                              .read(libraryControllerProvider.notifier)
                              .renamePlaylist(id, nextName);
                        },
                        onDelete: (id, name) async {
                          final confirmed = await _confirmDeletePlaylist(
                            context,
                            name: name,
                          );
                          if (!confirmed) return;
                          await ref
                              .read(libraryControllerProvider.notifier)
                              .deletePlaylist(id);
                        },
                        onCreate: () => _createPlaylist(context),
                        onRefreshPlugins: _refreshPluginPlaylists,
                        pluginLoading: _loadingPluginPlaylists,
                        pluginError: _pluginPlaylistError,
                        onClose: () =>
                            setState(() => _playlistsPanelOpen = false),
                        coverDir: coverDir,
                        displayName: (p) => _playlistDisplayName(l10n, p),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        );

        if (widget.useGlobalTopBar) {
          return Column(children: [content]);
        }

        return Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 6, 10, 6),
              child: SizedBox(
                height: 48,
                child: Row(
                  children: [
                    IconButton(
                      tooltip: l10n.playlistSectionTitle,
                      icon: const Icon(Icons.playlist_play),
                      onPressed: togglePlaylistsPanel,
                    ),
                    Expanded(
                      child: Text(
                        l10n.playlistSectionTitle,
                        style: theme.textTheme.headlineSmall?.copyWith(
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                    IconButton(
                      tooltip: l10n.playlistCreateTooltip,
                      icon: const Icon(Icons.playlist_add_outlined),
                      onPressed: createPlaylistFromTopBar,
                    ),
                  ],
                ),
              ),
            ),
            Divider(
              height: 1,
              thickness: 0.8,
              color: theme.colorScheme.onSurface.withValues(alpha: 0.12),
            ),
            content,
          ],
        );
      },
    );
  }

  void _ensurePlaylistSelected(List<PlaylistLite> playlists, int? selectedId) {
    if (_autoSelecting || selectedId != null || playlists.isEmpty) return;
    _autoSelecting = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final notifier = ref.read(libraryControllerProvider.notifier);
      final target = _defaultPlaylistId(playlists);
      notifier.selectPlaylist(target);
      _autoSelecting = false;
    });
  }

  int _defaultPlaylistId(List<PlaylistLite> playlists) {
    for (final p in playlists) {
      if (p.systemKey == 'liked') {
        return p.id.toInt();
      }
    }
    return playlists.first.id.toInt();
  }

  Future<void> _createPlaylist(BuildContext context) async {
    final l10n = AppLocalizations.of(context)!;
    final name = await _promptPlaylistName(
      context,
      title: l10n.playlistCreateTitle,
    );
    if (name == null) return;
    await ref.read(libraryControllerProvider.notifier).createPlaylist(name);
  }

  Future<String?> _promptPlaylistName(
    BuildContext context, {
    required String title,
    String initialValue = '',
  }) async {
    final l10n = AppLocalizations.of(context)!;
    final controller = TextEditingController(text: initialValue);
    try {
      final result = await showDialog<String>(
        context: context,
        builder: (context) {
          return AlertDialog(
            title: Text(title),
            content: TextField(
              controller: controller,
              autofocus: true,
              decoration: InputDecoration(
                border: const OutlineInputBorder(),
                hintText: l10n.playlistNameHint,
              ),
              onSubmitted: (value) {
                final trimmed = value.trim();
                Navigator.of(context).pop(trimmed.isEmpty ? null : trimmed);
              },
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: Text(l10n.cancel),
              ),
              FilledButton(
                onPressed: () {
                  final trimmed = controller.text.trim();
                  Navigator.of(context).pop(trimmed.isEmpty ? null : trimmed);
                },
                child: Text(l10n.ok),
              ),
            ],
          );
        },
      );
      return result;
    } finally {
      controller.dispose();
    }
  }

  Future<bool> _confirmDeletePlaylist(
    BuildContext context, {
    required String name,
  }) async {
    final l10n = AppLocalizations.of(context)!;
    final result = await showDialog<bool>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: Text(l10n.playlistDeleteTitle),
          content: Text(l10n.playlistDeleteConfirm(name)),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.playlistDeleteAction),
            ),
          ],
        );
      },
    );
    return result ?? false;
  }

  String _playlistDisplayName(AppLocalizations l10n, PlaylistLite playlist) {
    if (playlist.systemKey == 'liked') {
      return l10n.likedPlaylistName;
    }
    return playlist.name;
  }
}

class _UnifiedPlaylistsSidebar extends StatefulWidget {
  const _UnifiedPlaylistsSidebar({
    required this.localPlaylists,
    required this.selectedLocalPlaylistId,
    required this.pluginPlaylists,
    required this.selectedPluginPlaylistKey,
    required this.coverDir,
    required this.onSelectLocal,
    required this.onSelectPlugin,
    required this.onRenameLocal,
    required this.onDeleteLocal,
    required this.displayName,
    this.pluginError,
  });

  final List<PlaylistLite> localPlaylists;
  final int? selectedLocalPlaylistId;
  final List<_PluginPlaylistEntry> pluginPlaylists;
  final String? selectedPluginPlaylistKey;
  final String coverDir;
  final ValueChanged<int> onSelectLocal;
  final ValueChanged<_PluginPlaylistEntry> onSelectPlugin;
  final Future<void> Function(int id, String currentName) onRenameLocal;
  final Future<void> Function(int id, String currentName) onDeleteLocal;
  final String Function(PlaylistLite playlist) displayName;
  final String? pluginError;

  @override
  State<_UnifiedPlaylistsSidebar> createState() =>
      _UnifiedPlaylistsSidebarState();
}

class _UnifiedPlaylistsSidebarState extends State<_UnifiedPlaylistsSidebar> {
  final ScrollController _scrollController = ScrollController();

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final pluginGroups = <String, List<_PluginPlaylistEntry>>{};
    final pluginGroupHeaders = <String, String>{};
    for (final playlist in widget.pluginPlaylists) {
      final groupKey = '${playlist.pluginId}::${playlist.typeId}';
      pluginGroups
          .putIfAbsent(groupKey, () => <_PluginPlaylistEntry>[])
          .add(playlist);
      pluginGroupHeaders.putIfAbsent(
        groupKey,
        () => '${playlist.pluginName} / ${playlist.typeDisplayName}',
      );
    }

    return Scrollbar(
      controller: _scrollController,
      child: ListView(
        controller: _scrollController,
        primary: false,
        padding: const EdgeInsets.symmetric(vertical: 4),
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 6, 8, 6),
            child: Text('本地歌单', style: Theme.of(context).textTheme.labelLarge),
          ),
          for (final playlist in widget.localPlaylists)
            _PlaylistSidebarItem(
              coverDir: widget.coverDir,
              playlist: playlist,
              name: widget.displayName(playlist),
              subtitle: l10n.playlistTrackCount(playlist.trackCount.toInt()),
              isSelected:
                  widget.selectedPluginPlaylistKey == null &&
                  widget.selectedLocalPlaylistId == playlist.id.toInt(),
              isSystem: playlist.systemKey != null,
              onTap: () => widget.onSelectLocal(playlist.id.toInt()),
              onRename: () => widget.onRenameLocal(
                playlist.id.toInt(),
                widget.displayName(playlist),
              ),
              onDelete: () => widget.onDeleteLocal(
                playlist.id.toInt(),
                widget.displayName(playlist),
              ),
            ),
          const SizedBox(height: 8),
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 6, 8, 6),
            child: Text('插件歌单', style: Theme.of(context).textTheme.labelLarge),
          ),
          if (widget.pluginError != null &&
              widget.pluginError!.trim().isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 2, 10, 8),
              child: Text(
                widget.pluginError!,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.error,
                ),
              ),
            ),
          if (widget.pluginPlaylists.isEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 2, 10, 10),
              child: Text(
                '暂无插件歌单',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ),
          for (final groupKey in pluginGroups.keys) ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 4, 10, 4),
              child: Text(
                pluginGroupHeaders[groupKey] ?? groupKey,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            for (final playlist in pluginGroups[groupKey]!)
              _PluginPlaylistSidebarItem(
                playlist: playlist,
                isSelected: widget.selectedPluginPlaylistKey == playlist.key,
                onTap: () => widget.onSelectPlugin(playlist),
              ),
            const SizedBox(height: 4),
          ],
        ],
      ),
    );
  }
}

class _PluginPlaylistSidebarItem extends StatefulWidget {
  const _PluginPlaylistSidebarItem({
    required this.playlist,
    required this.isSelected,
    required this.onTap,
  });

  final _PluginPlaylistEntry playlist;
  final bool isSelected;
  final VoidCallback onTap;

  @override
  State<_PluginPlaylistSidebarItem> createState() =>
      _PluginPlaylistSidebarItemState();
}

class _PluginPlaylistSidebarItemState
    extends State<_PluginPlaylistSidebarItem> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = theme.colorScheme.primary;
    final hovered = _hovering && !widget.isSelected;
    final base = theme.colorScheme.surface.withValues(
      alpha: hovered ? 0.52 : 0.30,
    );
    final selectedBg = theme.colorScheme.secondaryContainer.withValues(
      alpha: 0.88,
    );
    final border = widget.isSelected
        ? accent.withValues(alpha: 0.45)
        : theme.colorScheme.onSurface.withValues(alpha: hovered ? 0.20 : 0.10);
    final subtitle =
        '${widget.playlist.sourceLabel}${widget.playlist.trackCount == null ? '' : ' · ${widget.playlist.trackCount}'}';

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: widget.isSelected
                  ? [
                      selectedBg,
                      theme.colorScheme.secondaryContainer.withValues(
                        alpha: 0.74,
                      ),
                    ]
                  : [
                      base,
                      theme.colorScheme.surfaceContainerHighest.withValues(
                        alpha: 0.28,
                      ),
                    ],
            ),
            border: Border.all(color: border),
            boxShadow: [
              if (widget.isSelected)
                BoxShadow(
                  color: accent.withValues(alpha: 0.18),
                  blurRadius: 14,
                  offset: const Offset(0, 4),
                )
              else if (hovered)
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.10),
                  blurRadius: 10,
                  offset: const Offset(0, 3),
                ),
            ],
          ),
          child: Material(
            type: MaterialType.transparency,
            child: InkWell(
              borderRadius: BorderRadius.circular(14),
              onTap: widget.onTap,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 8, 6, 8),
                child: Row(
                  children: [
                    _PluginPlaylistCover(
                      accent: accent,
                      cover: widget.playlist.cover,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Expanded(
                                child: Text(
                                  widget.playlist.title,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.titleSmall?.copyWith(
                                    fontWeight: widget.isSelected
                                        ? FontWeight.w700
                                        : FontWeight.w600,
                                  ),
                                ),
                              ),
                              if (widget.isSelected)
                                Icon(
                                  Icons.graphic_eq_rounded,
                                  size: 16,
                                  color: accent.withValues(alpha: 0.92),
                                ),
                            ],
                          ),
                          const SizedBox(height: 3),
                          Row(
                            children: [
                              Icon(
                                Icons.queue_music_rounded,
                                size: 13,
                                color: theme.colorScheme.onSurfaceVariant
                                    .withValues(alpha: 0.85),
                              ),
                              const SizedBox(width: 4),
                              Expanded(
                                child: Text(
                                  subtitle,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: theme.colorScheme.onSurfaceVariant,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _PluginPlaylistCover extends StatelessWidget {
  const _PluginPlaylistCover({required this.accent, this.cover});

  final Color accent;
  final QueueCover? cover;

  @override
  Widget build(BuildContext context) {
    final placeholder = Container(
      width: 44,
      height: 44,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(11),
        color: accent.withValues(alpha: 0.14),
        border: Border.all(color: accent.withValues(alpha: 0.22)),
      ),
      child: Icon(Icons.cloud_outlined, size: 20, color: accent),
    );

    return _CoverImageByRef(cover: cover, placeholder: placeholder, size: 44);
  }
}

class _PlaylistSidebarItem extends StatefulWidget {
  const _PlaylistSidebarItem({
    required this.coverDir,
    required this.playlist,
    required this.name,
    required this.subtitle,
    required this.isSelected,
    required this.isSystem,
    required this.onTap,
    required this.onRename,
    required this.onDelete,
  });

  final String coverDir;
  final PlaylistLite playlist;
  final String name;
  final String subtitle;
  final bool isSelected;
  final bool isSystem;
  final VoidCallback onTap;
  final Future<void> Function() onRename;
  final Future<void> Function() onDelete;

  @override
  State<_PlaylistSidebarItem> createState() => _PlaylistSidebarItemState();
}

class _PlaylistSidebarItemState extends State<_PlaylistSidebarItem> {
  bool _hovering = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final likedPlaylist = widget.playlist.systemKey == 'liked';
    final accent = likedPlaylist
        ? theme.colorScheme.error
        : theme.colorScheme.primary;
    final hovered = _hovering && !widget.isSelected;
    final base = theme.colorScheme.surface.withValues(
      alpha: hovered ? 0.52 : 0.30,
    );
    final selectedBg = theme.colorScheme.secondaryContainer.withValues(
      alpha: 0.88,
    );
    final border = widget.isSelected
        ? accent.withValues(alpha: 0.45)
        : theme.colorScheme.onSurface.withValues(alpha: hovered ? 0.20 : 0.10);

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovering = true),
        onExit: (_) => setState(() => _hovering = false),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: widget.isSelected
                  ? [
                      selectedBg,
                      theme.colorScheme.secondaryContainer.withValues(
                        alpha: 0.74,
                      ),
                    ]
                  : [
                      base,
                      theme.colorScheme.surfaceContainerHighest.withValues(
                        alpha: 0.28,
                      ),
                    ],
            ),
            border: Border.all(color: border),
            boxShadow: [
              if (widget.isSelected)
                BoxShadow(
                  color: accent.withValues(alpha: 0.18),
                  blurRadius: 14,
                  offset: const Offset(0, 4),
                )
              else if (hovered)
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.10),
                  blurRadius: 10,
                  offset: const Offset(0, 3),
                ),
            ],
          ),
          child: Material(
            type: MaterialType.transparency,
            child: InkWell(
              borderRadius: BorderRadius.circular(14),
              onTap: widget.onTap,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 8, 6, 8),
                child: Row(
                  children: [
                    _PlaylistCover(
                      coverDir: widget.coverDir,
                      firstTrackId: widget.playlist.firstTrackId?.toInt(),
                      likedPlaylist: likedPlaylist,
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Expanded(
                                child: Text(
                                  widget.name,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.titleSmall?.copyWith(
                                    fontWeight: widget.isSelected
                                        ? FontWeight.w700
                                        : FontWeight.w600,
                                  ),
                                ),
                              ),
                              if (widget.isSelected)
                                Icon(
                                  Icons.graphic_eq_rounded,
                                  size: 16,
                                  color: accent.withValues(alpha: 0.92),
                                ),
                            ],
                          ),
                          const SizedBox(height: 3),
                          Row(
                            children: [
                              Icon(
                                Icons.queue_music_rounded,
                                size: 13,
                                color: theme.colorScheme.onSurfaceVariant
                                    .withValues(alpha: 0.85),
                              ),
                              const SizedBox(width: 4),
                              Expanded(
                                child: Text(
                                  widget.subtitle,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: theme.colorScheme.onSurfaceVariant,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                    if (!widget.isSystem)
                      AnimatedOpacity(
                        duration: const Duration(milliseconds: 160),
                        opacity: widget.isSelected || _hovering ? 1.0 : 0.78,
                        child: PopupMenuButton<String>(
                          icon: const Icon(Icons.more_horiz_rounded, size: 18),
                          onSelected: (value) async {
                            if (value == 'rename') {
                              await widget.onRename();
                              return;
                            }
                            await widget.onDelete();
                          },
                          itemBuilder: (context) => [
                            PopupMenuItem(
                              value: 'rename',
                              child: Text(
                                AppLocalizations.of(
                                  context,
                                )!.playlistRenameAction,
                              ),
                            ),
                            PopupMenuItem(
                              value: 'delete',
                              child: Text(
                                AppLocalizations.of(
                                  context,
                                )!.playlistDeleteAction,
                              ),
                            ),
                          ],
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _PlaylistCover extends StatelessWidget {
  const _PlaylistCover({
    required this.coverDir,
    required this.firstTrackId,
    required this.likedPlaylist,
  });

  final String coverDir;
  final int? firstTrackId;
  final bool likedPlaylist;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final icon = likedPlaylist ? Icons.favorite : Icons.playlist_play;
    final iconColor = likedPlaylist
        ? theme.colorScheme.error
        : theme.colorScheme.primary;
    final placeholder = Container(
      width: 44,
      height: 44,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(11),
        color: iconColor.withValues(alpha: 0.14),
        border: Border.all(color: iconColor.withValues(alpha: 0.22)),
      ),
      child: Icon(icon, size: 20, color: iconColor),
    );

    if (firstTrackId == null || coverDir.isEmpty) {
      return placeholder;
    }

    final path = '$coverDir${Platform.pathSeparator}$firstTrackId';
    final provider = ResizeImage(
      FileImage(File(path)),
      width: 96,
      height: 96,
      allowUpscaling: false,
    );
    return ClipRRect(
      borderRadius: BorderRadius.circular(11),
      child: Image(
        image: provider,
        width: 44,
        height: 44,
        fit: BoxFit.cover,
        filterQuality: FilterQuality.low,
        gaplessPlayback: true,
        errorBuilder: (context, error, stackTrace) => placeholder,
      ),
    );
  }
}

class _PlaylistsDrawerPanel extends StatelessWidget {
  const _PlaylistsDrawerPanel({
    required this.playlists,
    required this.selectedPlaylistId,
    required this.pluginPlaylists,
    required this.selectedPluginPlaylistKey,
    required this.coverDir,
    required this.onSelect,
    required this.onSelectPlugin,
    required this.onRename,
    required this.onDelete,
    required this.onCreate,
    required this.onRefreshPlugins,
    required this.pluginLoading,
    required this.pluginError,
    required this.onClose,
    required this.displayName,
  });

  final List<PlaylistLite> playlists;
  final int? selectedPlaylistId;
  final List<_PluginPlaylistEntry> pluginPlaylists;
  final String? selectedPluginPlaylistKey;
  final String coverDir;
  final ValueChanged<int> onSelect;
  final ValueChanged<_PluginPlaylistEntry> onSelectPlugin;
  final Future<void> Function(int id, String currentName) onRename;
  final Future<void> Function(int id, String currentName) onDelete;
  final VoidCallback onCreate;
  final Future<void> Function() onRefreshPlugins;
  final bool pluginLoading;
  final String? pluginError;
  final VoidCallback onClose;
  final String Function(PlaylistLite playlist) displayName;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);

    return SafeArea(
      right: false,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 12, 10, 12),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(18),
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
            child: DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                  colors: [
                    theme.colorScheme.surface.withValues(alpha: 0.84),
                    theme.colorScheme.surfaceContainerHigh.withValues(
                      alpha: 0.76,
                    ),
                  ],
                ),
                border: Border.all(
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.14),
                ),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withValues(alpha: 0.14),
                    blurRadius: 24,
                    offset: const Offset(0, 10),
                  ),
                ],
              ),
              child: Column(
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(12, 10, 8, 6),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            l10n.playlistSectionTitle,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: theme.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                        IconButton(
                          tooltip: l10n.playlistCreateTooltip,
                          onPressed: onCreate,
                          icon: const Icon(Icons.playlist_add_outlined),
                        ),
                        IconButton(
                          tooltip: '刷新插件歌单',
                          onPressed: pluginLoading
                              ? null
                              : () => onRefreshPlugins(),
                          icon: pluginLoading
                              ? const SizedBox(
                                  width: 16,
                                  height: 16,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.cloud_sync_outlined),
                        ),
                        IconButton(
                          tooltip: l10n.tooltipBack,
                          onPressed: onClose,
                          icon: const Icon(Icons.close),
                        ),
                      ],
                    ),
                  ),
                  Divider(
                    height: 1,
                    thickness: 0.8,
                    color: theme.colorScheme.onSurface.withValues(alpha: 0.12),
                  ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(8, 8, 8, 10),
                      child: _UnifiedPlaylistsSidebar(
                        localPlaylists: playlists,
                        selectedLocalPlaylistId: selectedPlaylistId,
                        pluginPlaylists: pluginPlaylists,
                        selectedPluginPlaylistKey: selectedPluginPlaylistKey,
                        coverDir: coverDir,
                        onSelectLocal: onSelect,
                        onSelectPlugin: onSelectPlugin,
                        onRenameLocal: onRename,
                        onDeleteLocal: onDelete,
                        displayName: displayName,
                        pluginError: pluginError,
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
  }
}

class _PlaylistTracksPane extends StatelessWidget {
  const _PlaylistTracksPane({
    required this.searchController,
    required this.queueSourceLabel,
    required this.selectedLabel,
    required this.playlists,
    required this.selectedPlaylistId,
    required this.results,
    required this.likedTrackIds,
    required this.coverDir,
    required this.onSearchChanged,
    required this.onActivate,
    required this.onEnqueue,
    required this.onSetLiked,
    required this.onAddToPlaylist,
    required this.onRemoveFromPlaylist,
    required this.blockedReasonByTrackId,
    required this.onViewportRangeChanged,
    this.onMoveInCurrentPlaylist,
    this.onBatchAddToPlaylist,
    this.onBatchRemoveFromCurrentPlaylist,
  });

  final TextEditingController searchController;
  final String queueSourceLabel;
  final String selectedLabel;
  final List<PlaylistLite> playlists;
  final int? selectedPlaylistId;
  final List<TrackLite> results;
  final Set<int> likedTrackIds;
  final String coverDir;
  final ValueChanged<String> onSearchChanged;
  final Future<void> Function(int index, List<TrackLite> items) onActivate;
  final Future<void> Function(TrackLite track) onEnqueue;
  final Future<void> Function(TrackLite track, bool liked) onSetLiked;
  final Future<void> Function(TrackLite track, int playlistId) onAddToPlaylist;
  final Future<void> Function(TrackLite track, int playlistId)
  onRemoveFromPlaylist;
  final Map<int, String> blockedReasonByTrackId;
  final void Function(int startIndex, int endIndex) onViewportRangeChanged;
  final Future<void> Function(TrackLite track, int newIndex)?
  onMoveInCurrentPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchAddToPlaylist;
  final Future<void> Function(List<TrackLite> tracks, int playlistId)?
  onBatchRemoveFromCurrentPlaylist;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          controller: searchController,
          decoration: InputDecoration(
            prefixIcon: const Icon(Icons.search),
            hintText: l10n.searchHint,
            filled: true,
            fillColor: theme.colorScheme.surfaceContainerLowest.withValues(
              alpha: 0.72,
            ),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(14),
              borderSide: BorderSide(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.10),
              ),
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(14),
              borderSide: BorderSide(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.10),
              ),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(14),
              borderSide: BorderSide(color: theme.colorScheme.primary),
            ),
          ),
          onChanged: onSearchChanged,
        ),
        const SizedBox(height: 12),
        Container(
          width: double.infinity,
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          decoration: BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: [
                theme.colorScheme.surfaceContainerHigh.withValues(alpha: 0.74),
                theme.colorScheme.surfaceContainer.withValues(alpha: 0.58),
              ],
            ),
            border: Border.all(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.08),
            ),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.045),
                blurRadius: 8,
                offset: const Offset(0, 2),
              ),
            ],
            borderRadius: BorderRadius.circular(14),
          ),
          child: Row(
            children: [
              Icon(
                Icons.queue_music,
                size: 18,
                color: theme.colorScheme.primary,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      l10n.queueSourceTitle,
                      style: theme.textTheme.labelMedium,
                    ),
                    Text(
                      queueSourceLabel,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodyMedium,
                    ),
                    Text(
                      l10n.queueSourceHint,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        Text(
          selectedLabel,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: theme.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 12),
        Expanded(
          child: TrackList(
            coverDir: coverDir,
            items: results,
            likedTrackIds: likedTrackIds,
            playlists: playlists,
            currentPlaylistId: selectedPlaylistId,
            onActivate: onActivate,
            onEnqueue: onEnqueue,
            onSetLiked: onSetLiked,
            onAddToPlaylist: onAddToPlaylist,
            onRemoveFromPlaylist: onRemoveFromPlaylist,
            onMoveInCurrentPlaylist: onMoveInCurrentPlaylist,
            onBatchAddToPlaylist: onBatchAddToPlaylist,
            onBatchRemoveFromCurrentPlaylist: onBatchRemoveFromCurrentPlaylist,
            blockedReasonByTrackId: blockedReasonByTrackId,
            onViewportRangeChanged: onViewportRangeChanged,
          ),
        ),
      ],
    );
  }
}

class _PluginPlaylistTracksPane extends StatefulWidget {
  const _PluginPlaylistTracksPane({
    required this.searchController,
    required this.queueSourceLabel,
    required this.selectedLabel,
    required this.sourceLabel,
    required this.tracks,
    required this.loading,
    required this.loadingMore,
    required this.hasMore,
    required this.filterActive,
    required this.error,
    required this.onSearchChanged,
    required this.onLoadMore,
    required this.onActivate,
    required this.onEnqueue,
    this.onPlayAll,
    this.onEnqueueAll,
  });

  final TextEditingController searchController;
  final String queueSourceLabel;
  final String selectedLabel;
  final String sourceLabel;
  final List<QueueItem> tracks;
  final bool loading;
  final bool loadingMore;
  final bool hasMore;
  final bool filterActive;
  final String? error;
  final ValueChanged<String> onSearchChanged;
  final Future<void> Function() onLoadMore;
  final Future<void> Function(int index, List<QueueItem> items) onActivate;
  final Future<void> Function(QueueItem item) onEnqueue;
  final Future<void> Function()? onPlayAll;
  final Future<void> Function()? onEnqueueAll;

  @override
  State<_PluginPlaylistTracksPane> createState() =>
      _PluginPlaylistTracksPaneState();
}

class _PluginPlaylistTracksPaneState extends State<_PluginPlaylistTracksPane> {
  static const double _loadMoreThreshold = 320;
  static const double _itemExtent = 70;
  final ScrollController _scrollController = ScrollController();
  Future<void>? _pendingLoadMore;
  Timer? _settleTimer;
  bool _deferHeavy = false;
  double _lastPixels = 0.0;
  int _lastMicros = 0;

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);
    _schedulePrefetchCheck();
  }

  @override
  void didUpdateWidget(covariant _PluginPlaylistTracksPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.tracks.length != widget.tracks.length ||
        oldWidget.hasMore != widget.hasMore ||
        oldWidget.loading != widget.loading ||
        oldWidget.loadingMore != widget.loadingMore ||
        oldWidget.filterActive != widget.filterActive) {
      _schedulePrefetchCheck();
    }
  }

  @override
  void dispose() {
    _settleTimer?.cancel();
    _scrollController.removeListener(_onScroll);
    _scrollController.dispose();
    super.dispose();
  }

  bool get _canLoadMore {
    return !widget.filterActive &&
        widget.hasMore &&
        !widget.loading &&
        !widget.loadingMore &&
        _pendingLoadMore == null;
  }

  void _onScroll() {
    if (!_scrollController.hasClients || !_canLoadMore) return;
    if (_scrollController.position.extentAfter <= _loadMoreThreshold) {
      _triggerLoadMore();
    }
  }

  void _schedulePrefetchCheck() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_scrollController.hasClients || !_canLoadMore) return;
      if (_scrollController.position.maxScrollExtent <= 0) {
        _triggerLoadMore();
      }
    });
  }

  void _triggerLoadMore() {
    if (!_canLoadMore) return;
    final pending = widget.onLoadMore();
    _pendingLoadMore = pending;
    pending.whenComplete(() {
      if (!mounted || !identical(_pendingLoadMore, pending)) return;
      _pendingLoadMore = null;
      _schedulePrefetchCheck();
    });
  }

  bool _onScrollNotification(ScrollNotification n) {
    final nowMicros = DateTime.now().microsecondsSinceEpoch;
    final pixels = n.metrics.pixels;
    final dtMicros = _lastMicros == 0 ? 0 : (nowMicros - _lastMicros);
    final deltaPx = (pixels - _lastPixels).abs();
    final dtMs = dtMicros / 1000.0;
    final speed = dtMs <= 0 ? 0.0 : (deltaPx / dtMs); // px/ms
    _lastMicros = nowMicros;
    _lastPixels = pixels;

    final viewport = n.metrics.viewportDimension;
    final isFast = deltaPx > viewport * 0.60 || speed > 5.0; // ~5000 px/s
    if (isFast && !_deferHeavy) {
      setState(() => _deferHeavy = true);
    }

    _settleTimer?.cancel();
    _settleTimer = Timer(const Duration(milliseconds: 160), () {
      if (!mounted || !_deferHeavy) return;
      setState(() => _deferHeavy = false);
    });
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final theme = Theme.of(context);
    final hasPendingRows =
        widget.loadingMore || (!widget.filterActive && widget.hasMore);
    final listCount = widget.tracks.length;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          controller: widget.searchController,
          decoration: InputDecoration(
            prefixIcon: const Icon(Icons.search),
            hintText: l10n.searchHint,
            filled: true,
            fillColor: theme.colorScheme.surfaceContainerLowest.withValues(
              alpha: 0.72,
            ),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(14),
              borderSide: BorderSide(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.10),
              ),
            ),
          ),
          onChanged: widget.onSearchChanged,
        ),
        const SizedBox(height: 12),
        Text(
          widget.selectedLabel,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: theme.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 4),
        Text(
          widget.sourceLabel,
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            FilledButton.icon(
              onPressed: widget.onPlayAll == null
                  ? null
                  : () => widget.onPlayAll!(),
              icon: const Icon(Icons.play_arrow),
              label: const Text('Play All'),
            ),
            const SizedBox(width: 8),
            OutlinedButton.icon(
              onPressed: widget.onEnqueueAll == null
                  ? null
                  : () => widget.onEnqueueAll!(),
              icon: const Icon(Icons.queue_music),
              label: const Text('Enqueue All'),
            ),
            const Spacer(),
            Text(
              hasPendingRows ? '$listCount+' : '$listCount',
              style: theme.textTheme.bodySmall,
            ),
          ],
        ),
        const SizedBox(height: 10),
        Expanded(
          child: widget.loading
              ? const Center(child: CircularProgressIndicator())
              : (widget.error != null && widget.error!.trim().isNotEmpty)
              ? Center(
                  child: Text(
                    widget.error!,
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: theme.colorScheme.error,
                    ),
                  ),
                )
              : widget.tracks.isEmpty
              ? Center(child: Text(l10n.noResultsHint))
              : Stack(
                  children: [
                    NotificationListener<ScrollNotification>(
                      onNotification: _onScrollNotification,
                      child: ListView.builder(
                        controller: _scrollController,
                        itemExtent: _itemExtent,
                        cacheExtent: _deferHeavy ? 180 : 760,
                        itemCount: widget.tracks.length,
                        itemBuilder: (context, index) {
                          final item = widget.tracks[index];
                          final title = item.title?.trim().isNotEmpty == true
                              ? item.title!.trim()
                              : item.track.trackId;
                          final artist = item.artist?.trim() ?? '';
                          final album = item.album?.trim() ?? '';
                          final subtitle = artist.isEmpty
                              ? album
                              : (album.isEmpty ? artist : '$artist · $album');
                          return DecoratedBox(
                            decoration: BoxDecoration(
                              border: Border(
                                bottom: BorderSide(
                                  color: Theme.of(
                                    context,
                                  ).dividerColor.withValues(alpha: 0.55),
                                  width: 0.7,
                                ),
                              ),
                            ),
                            child: ListTile(
                              dense: true,
                              contentPadding: const EdgeInsets.symmetric(
                                horizontal: 12,
                              ),
                              leading: _PluginTrackCover(
                                cover: item.cover,
                                deferHeavy: _deferHeavy,
                              ),
                              title: Text(
                                title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                              subtitle: _deferHeavy || subtitle.isEmpty
                                  ? null
                                  : Text(
                                      subtitle,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                    ),
                              onTap: () =>
                                  widget.onActivate(index, widget.tracks),
                              trailing: _deferHeavy
                                  ? const SizedBox(width: 24)
                                  : IconButton(
                                      tooltip: 'Enqueue',
                                      onPressed: () => widget.onEnqueue(item),
                                      icon: const Icon(
                                        Icons.add_to_queue_outlined,
                                      ),
                                    ),
                            ),
                          );
                        },
                      ),
                    ),
                    if (widget.loadingMore)
                      Positioned(
                        left: 0,
                        right: 0,
                        bottom: 8,
                        child: Center(
                          child: Container(
                            padding: const EdgeInsets.all(8),
                            decoration: BoxDecoration(
                              color: theme.colorScheme.surface.withValues(
                                alpha: 0.92,
                              ),
                              borderRadius: BorderRadius.circular(999),
                              border: Border.all(
                                color: theme.colorScheme.onSurface.withValues(
                                  alpha: 0.10,
                                ),
                              ),
                            ),
                            child: const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
        ),
        const SizedBox(height: 6),
        Text(
          widget.queueSourceLabel,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }
}

class _PluginTrackCover extends StatefulWidget {
  const _PluginTrackCover({this.cover, required this.deferHeavy});

  final QueueCover? cover;
  final bool deferHeavy;

  @override
  State<_PluginTrackCover> createState() => _PluginTrackCoverState();
}

class _PluginTrackCoverState extends State<_PluginTrackCover> {
  static const Duration _lazyDelay = Duration(milliseconds: 90);
  Timer? _timer;
  bool _showCover = false;
  String? _coverToken;

  @override
  void initState() {
    super.initState();
    if (widget.deferHeavy) {
      _showCover = false;
      return;
    }
    _refreshCoverVisibility(initial: true);
  }

  @override
  void didUpdateWidget(covariant _PluginTrackCover oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.deferHeavy) {
      _timer?.cancel();
      if (_showCover) {
        setState(() => _showCover = false);
      }
      return;
    }
    if (oldWidget.deferHeavy && !widget.deferHeavy) {
      _refreshCoverVisibility();
      return;
    }
    if (_coverIdentity(oldWidget.cover) != _coverIdentity(widget.cover)) {
      _refreshCoverVisibility();
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  String? _coverIdentity(QueueCover? cover) {
    if (cover == null) return null;
    return '${cover.kind.name}:${cover.value}';
  }

  void _refreshCoverVisibility({bool initial = false}) {
    _timer?.cancel();
    final token = _coverIdentity(widget.cover);
    _coverToken = token;
    if (token == null) {
      if (initial) {
        _showCover = false;
      } else {
        setState(() => _showCover = false);
      }
      return;
    }
    if (initial) {
      _showCover = false;
    } else {
      setState(() => _showCover = false);
    }
    _timer = Timer(_lazyDelay, () {
      if (!mounted || _coverToken != token) return;
      setState(() => _showCover = true);
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final placeholder = Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(8),
        color: theme.colorScheme.primary.withValues(alpha: 0.12),
        border: Border.all(
          color: theme.colorScheme.primary.withValues(alpha: 0.22),
        ),
      ),
      child: Icon(
        Icons.music_note_rounded,
        size: 18,
        color: theme.colorScheme.primary,
      ),
    );

    if (widget.deferHeavy || !_showCover) return placeholder;
    return _CoverImageByRef(
      cover: widget.cover,
      placeholder: placeholder,
      size: 40,
    );
  }
}

class _CoverImageByRef extends StatelessWidget {
  const _CoverImageByRef({
    required this.cover,
    required this.placeholder,
    required this.size,
  });

  final QueueCover? cover;
  final Widget placeholder;
  final double size;

  @override
  Widget build(BuildContext context) {
    final ref = cover;
    if (ref == null) return placeholder;
    final radius = BorderRadius.circular(size == 44 ? 11 : 8);
    switch (ref.kind) {
      case QueueCoverKind.url:
        return ClipRRect(
          borderRadius: radius,
          child: Image.network(
            ref.value,
            width: size,
            height: size,
            fit: BoxFit.cover,
            filterQuality: FilterQuality.low,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.file:
        return ClipRRect(
          borderRadius: radius,
          child: Image.file(
            File(ref.value),
            width: size,
            height: size,
            fit: BoxFit.cover,
            filterQuality: FilterQuality.low,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
      case QueueCoverKind.data:
        final bytes = _decodeCoverBytes(ref.value);
        if (bytes == null) return placeholder;
        return ClipRRect(
          borderRadius: radius,
          child: Image.memory(
            bytes,
            width: size,
            height: size,
            fit: BoxFit.cover,
            filterQuality: FilterQuality.low,
            gaplessPlayback: true,
            errorBuilder: (context, error, stackTrace) => placeholder,
          ),
        );
    }
  }

  Uint8List? _decodeCoverBytes(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return null;
    final data = () {
      if (text.startsWith('data:')) {
        final comma = text.indexOf(',');
        if (comma <= 0 || comma >= text.length - 1) return '';
        return text.substring(comma + 1);
      }
      return text;
    }();
    if (data.isEmpty) return null;
    try {
      return base64Decode(data);
    } catch (_) {
      return null;
    }
  }
}
