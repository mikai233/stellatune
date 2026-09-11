import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/library/catalog_bridge.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/playlists/models/playlists_data_models.dart';

class PluginPlaylistRefreshResult {
  const PluginPlaylistRefreshResult({
    required this.entries,
    required this.sourceErrors,
  });
  final List<PluginPlaylistEntry> entries;
  final List<String> sourceErrors;
  String? get aggregatedError =>
      sourceErrors.isEmpty ? null : sourceErrors.join('\n');
}

class PlaylistsPluginBridgeService {
  PlaylistsPluginBridgeService(this.catalog);
  final CatalogBridge catalog;
  Future<PluginPlaylistRefreshResult> fetchPlaylists({
    required PlayerBridge bridge,
  }) async {
    final sources = await catalog.sources();
    final entries = <PluginPlaylistEntry>[];
    final errors = <String>[];
    for (final source in sources.where((s) => !s.local)) {
      if (!source.available) {
        errors.add('${source.name}: ${source.error}');
        continue;
      }
      if (!source.browseKinds.contains(MediaKind.playlist)) continue;
      try {
        String? cursor;
        final seen = <String>{};
        do {
          final page = await catalog.browse(
            CatalogQuery(
              sourceInstanceId: source.id,
              kind: MediaKind.playlist,
              search: '',
              sort: CatalogSort.default_,
              cursor: cursor,
              limit: 200,
            ),
          );
          for (final item in page.items) {
            entries.add(
              PluginPlaylistEntry(
                key: '${source.id}::${item.reference.id}',
                pluginId: source.id,
                pluginName: source.name,
                typeId: 'media-library',
                typeDisplayName: source.name,
                sourceId: source.id,
                title: item.title,
                playlistId: item.reference.id,
                sourceLabel: source.name,
                trackCount: item.trackCount?.toInt(),
                cover: item.artworkUrl == null
                    ? null
                    : QueueCover(
                        kind: QueueCoverKind.url,
                        value: item.artworkUrl!,
                      ),
                playlistRef: item.reference,
              ),
            );
          }
          cursor = page.nextCursor;
          if (cursor != null && !seen.add(cursor)) {
            throw StateError('Catalog cursor cycle');
          }
        } while (cursor != null);
      } catch (e) {
        errors.add('${source.name}: $e');
      }
    }
    return PluginPlaylistRefreshResult(entries: entries, sourceErrors: errors);
  }

  Future<PluginTrackPage> fetchTrackPage({
    required PlayerBridge bridge,
    required PluginPlaylistEntry entry,
    required int pageSize,
    required int offset,
    int? limit,
    String? cursor,
  }) async {
    if (offset > 0 && cursor == null) {
      throw StateError('Missing catalog cursor');
    }
    final page = await catalog.browse(
      CatalogQuery(
        sourceInstanceId: entry.sourceId,
        kind: MediaKind.track,
        parent: MediaRef(
          sourceInstanceId: entry.sourceId,
          kind: MediaKind.playlist,
          id: entry.playlistId,
        ),
        search: '',
        sort: CatalogSort.default_,
        cursor: cursor,
        limit: (limit ?? pageSize).clamp(1, 200),
      ),
    );
    return PluginTrackPage(
      items: [
        for (final item in page.items)
          QueueItem(
            trackId: null,
            path: '',
            local: false,
            catalogItem: item,
            title: item.title,
            artist: item.artist,
            album: item.album,
            durationMs: item.durationMs?.toInt(),
            cover: item.artworkUrl == null
                ? null
                : QueueCover(kind: QueueCoverKind.url, value: item.artworkUrl!),
          ),
      ],
      fetchedCount: page.items.length,
      hasMore: page.nextCursor != null,
      nextCursor: page.nextCursor,
    );
  }
}
