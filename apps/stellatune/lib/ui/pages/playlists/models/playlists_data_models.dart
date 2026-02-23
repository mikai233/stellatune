import 'package:stellatune/player/queue_models.dart';

class PluginPlaylistEntry {
  const PluginPlaylistEntry({
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

class SparseTrackCacheEntry<T> {
  const SparseTrackCacheEntry({
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

class SparseTrackPage<T> {
  const SparseTrackPage({
    required this.items,
    required this.fetchedCount,
    required this.hasMore,
  });

  final List<T> items;
  final int fetchedCount;
  final bool hasMore;
}

abstract class SparseTrackSource<T> {
  const SparseTrackSource();

  String get cacheKey;
  int get pageSize;
  int get eagerLoadThreshold;
  int? get knownTotalCount;
  bool get eagerPreferred =>
      knownTotalCount == null || knownTotalCount! <= eagerLoadThreshold;

  Future<SparseTrackPage<T>> fetchPage({required int offset, int? limit});
}

class InMemorySparseTrackSource<T> extends SparseTrackSource<T> {
  const InMemorySparseTrackSource({
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
  Future<SparseTrackPage<T>> fetchPage({
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
    return SparseTrackPage<T>(
      items: slice,
      fetchedCount: fetchedCount,
      hasMore: end < items.length,
    );
  }
}

class PluginSparseTrackSource extends SparseTrackSource<QueueItem> {
  const PluginSparseTrackSource({
    required this.entry,
    required this.pageSize,
    required this.eagerLoadThreshold,
    required this.fetcher,
  });

  final PluginPlaylistEntry entry;
  @override
  final int pageSize;
  @override
  final int eagerLoadThreshold;
  final Future<SparseTrackPage<QueueItem>> Function({
    required int offset,
    int? limit,
  })
  fetcher;

  @override
  String get cacheKey => entry.key;

  @override
  int? get knownTotalCount => entry.trackCount;

  @override
  Future<SparseTrackPage<QueueItem>> fetchPage({
    required int offset,
    int? limit,
  }) {
    return fetcher(offset: offset, limit: limit);
  }
}
