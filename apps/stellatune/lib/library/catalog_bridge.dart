import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:stellatune/bridge/api/media_catalog.dart' as api;
import 'package:stellatune/bridge/third_party/stellatune_library/catalog.dart';
import 'package:stellatune/player/queue_models.dart';

export 'package:stellatune/bridge/third_party/stellatune_library/catalog.dart';

final catalogBridgeProvider = Provider<CatalogBridge>((ref) => CatalogBridge());

class CatalogBridge {
  Future<List<LibrarySource>> sources() => api.catalogListSources();
  Future<CatalogPage> browse(CatalogQuery query) =>
      api.catalogBrowse(query: query);
  Future<CatalogItem> detail(MediaRef reference) =>
      api.catalogGetDetail(reference: reference);
  Future<List<CatalogItem>> collect(CatalogQuery query, String requestId) =>
      api.catalogCollectTracks(query: query, requestId: requestId);
  Future<void> cancel(String requestId) =>
      api.catalogCancelCollection(requestId: requestId);
  Future<List<QueueItem>> prepare(List<CatalogItem> items) async {
    final ids = await api.catalogPrepareTracks(items: items);
    if (ids.length != items.length) {
      throw StateError('Incomplete track registration');
    }
    return [
      for (var i = 0; i < items.length; i++)
        QueueItem(
          catalogItem: items[i],
          trackId: ids[i],
          path: items[i].localPath ?? '',
          local: items[i].localPath != null,
          id: items[i].localTrackId?.toInt(),
          title: items[i].title,
          artist: items[i].artist,
          album: items[i].album,
          durationMs: items[i].durationMs?.toInt(),
          cover: items[i].artworkUrl == null
              ? null
              : QueueCover(
                  kind: QueueCoverKind.url,
                  value: items[i].artworkUrl!,
                ),
        ),
    ];
  }
}
