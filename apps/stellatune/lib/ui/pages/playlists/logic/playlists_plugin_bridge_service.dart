import 'dart:convert';

import 'package:stellatune/app/logging.dart';
import 'package:stellatune/app/settings_store.dart';
import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/queue_models.dart';
import 'package:stellatune/ui/pages/playlists/logic/playlists_plugin_value_utils.dart';
import 'package:stellatune/ui/pages/playlists/models/playlists_data_models.dart';

class PluginPlaylistRefreshResult {
  const PluginPlaylistRefreshResult({
    required this.entries,
    required this.sourceErrors,
  });

  final List<PluginPlaylistEntry> entries;
  final List<String> sourceErrors;

  String? get aggregatedError {
    if (entries.isNotEmpty || sourceErrors.isEmpty) {
      return null;
    }
    final preview = sourceErrors.take(3).join(' | ');
    final suffix = sourceErrors.length > 3
        ? ' | ...(${sourceErrors.length - 3} more)'
        : '';
    return '$preview$suffix';
  }
}

class PlaylistsPluginBridgeService {
  const PlaylistsPluginBridgeService();

  Future<PluginPlaylistRefreshResult> fetchPlaylists({
    required PlayerBridge bridge,
    required SettingsState settings,
  }) async {
    final sourceTypes = await bridge.sourceListTypes();
    logger.d('plugin playlists refresh: source_types=${sourceTypes.length}');
    final merged = <PluginPlaylistEntry>[];
    final seen = <String>{};
    final sourceErrors = <String>[];

    for (final source in sourceTypes) {
      final configJson = settings.sourceConfigFor(
        pluginId: source.pluginId,
        typeId: source.typeId,
        defaultValue: source.defaultConfigJson,
      );
      final config = PlaylistsPluginValueUtils.decodeJsonObjectOrEmpty(
        configJson,
      );
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
        final parsed = _parsePlaylistRow(
          row,
          source: source,
          config: config,
          seenKeys: seen,
        );
        if (parsed != null) {
          merged.add(parsed);
        }
      }

      final added = merged.length - beforeCount;
      logger.d(
        'plugin playlists refresh: plugin=${source.pluginId} type=${source.typeId} total_rows=${decoded.length} added=$added',
      );
    }

    logger.d(
      'plugin playlists refresh done: playlists=${merged.length} errors=${sourceErrors.length}',
    );
    return PluginPlaylistRefreshResult(
      entries: List<PluginPlaylistEntry>.unmodifiable(merged),
      sourceErrors: List<String>.unmodifiable(sourceErrors),
    );
  }

  Future<SparseTrackPage<QueueItem>> fetchTrackPage({
    required PlayerBridge bridge,
    required PluginPlaylistEntry entry,
    required int pageSize,
    required int offset,
    int? limit,
  }) async {
    final pageLimit = (limit ?? pageSize).clamp(1, 1000);
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
    return SparseTrackPage<QueueItem>(
      items: items,
      fetchedCount: fetchedCount,
      hasMore: hasMore,
    );
  }

  PluginPlaylistEntry? _parsePlaylistRow(
    Object? row, {
    required SourceCatalogTypeDescriptor source,
    required Map<String, Object?> config,
    required Set<String> seenKeys,
  }) {
    if (row is! Map) return null;
    final map = row.cast<Object?, Object?>();
    final kind = PlaylistsPluginValueUtils.asText(map['kind'])?.toLowerCase();
    if (kind != null && kind != 'playlist') return null;

    final playlistId =
        PlaylistsPluginValueUtils.asText(map['playlist_id']) ??
        PlaylistsPluginValueUtils.asText(map['item_id']) ??
        PlaylistsPluginValueUtils.asText(map['id']);
    if (playlistId == null || playlistId.isEmpty) return null;

    final title =
        PlaylistsPluginValueUtils.asText(map['title']) ??
        PlaylistsPluginValueUtils.asText(map['name']) ??
        playlistId;
    final sourceId =
        PlaylistsPluginValueUtils.asText(map['source_id']) ?? source.typeId;
    final sourceLabel =
        PlaylistsPluginValueUtils.asText(map['source_label']) ??
        '${source.pluginName} / ${source.displayName}';
    final key = '${source.pluginId}::${source.typeId}::$playlistId';
    if (!seenKeys.add(key)) return null;

    return PluginPlaylistEntry(
      key: key,
      pluginId: source.pluginId,
      pluginName: source.pluginName,
      typeId: source.typeId,
      typeDisplayName: source.displayName,
      sourceId: sourceId,
      title: title,
      playlistId: playlistId,
      sourceLabel: sourceLabel,
      trackCount: PlaylistsPluginValueUtils.asInt(map['track_count']),
      cover: PlaylistsPluginValueUtils.asCover(map['cover']),
      playlistRef: map['playlist_ref'],
      config: config,
    );
  }

  List<QueueItem> _parsePluginQueueItems(
    dynamic decoded,
    PluginPlaylistEntry entry,
  ) {
    final items = <QueueItem>[];
    if (decoded is! List) return items;
    for (final row in decoded) {
      if (row is! Map) continue;
      final map = row.cast<Object?, Object?>();
      final kind = PlaylistsPluginValueUtils.asText(map['kind'])?.toLowerCase();
      if (kind != null && kind != 'track') continue;

      final trackObj = map['track'];
      if (trackObj is! Map) continue;
      final track = trackObj.cast<String, Object?>();

      final sourceId =
          PlaylistsPluginValueUtils.asText(map['source_id']) ?? entry.sourceId;
      final trackId =
          PlaylistsPluginValueUtils.asText(map['track_id']) ??
          PlaylistsPluginValueUtils.asText(track['song_id']) ??
          '';
      if (trackId.isEmpty) continue;
      final extHint = PlaylistsPluginValueUtils.asText(map['ext_hint']) ?? '';
      final pathHint = PlaylistsPluginValueUtils.asText(map['path_hint']) ?? '';
      final decoderPluginId = PlaylistsPluginValueUtils.asText(
        map['decoder_plugin_id'],
      );
      final title =
          PlaylistsPluginValueUtils.asText(map['title']) ??
          PlaylistsPluginValueUtils.asText(track['title']);
      final artist =
          PlaylistsPluginValueUtils.asText(map['artist']) ??
          PlaylistsPluginValueUtils.asText(track['artist']);
      final album =
          PlaylistsPluginValueUtils.asText(map['album']) ??
          PlaylistsPluginValueUtils.asText(track['album']);
      final durationMs =
          PlaylistsPluginValueUtils.asInt(map['duration_ms']) ??
          PlaylistsPluginValueUtils.asInt(track['duration_ms']);
      final cover =
          PlaylistsPluginValueUtils.asCover(map['cover']) ??
          PlaylistsPluginValueUtils.asCover(track['cover']);

      items.add(
        QueueItem(
          trackId: null,
          path: pathHint,
          providerTrack: ProviderQueueTrack(
            providerId: sourceId,
            pluginId: entry.pluginId,
            typeId: entry.typeId,
            configJson: jsonEncode(entry.config),
            providerKey: trackId,
            pathHint: pathHint.isEmpty ? '$sourceId:$trackId.$extHint' : pathHint,
            sourcePluginId: entry.pluginId,
            decoderPluginId: decoderPluginId,
          ),
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
}
