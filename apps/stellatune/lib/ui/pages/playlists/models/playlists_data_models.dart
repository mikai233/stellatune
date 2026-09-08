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
  final int? trackCount;
  final QueueCover? cover;
  final Object? playlistRef;
}

class PluginTrackPage {
  const PluginTrackPage({
    required this.items,
    required this.fetchedCount,
    required this.hasMore,
  });

  final List<QueueItem> items;
  final int fetchedCount;
  final bool hasMore;
}
