import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/queue_models.dart';

class PlaybackResumeQueueUtils {
  static Future<List<TrackLite>> loadTracksForSource({
    required LibraryBridge bridge,
    required QueueSource source,
  }) {
    return switch (source.type) {
      QueueSourceType.folder => bridge.listTracks(
        folder: source.folderPath ?? '',
        recursive: source.includeSubfolders,
        query: '',
      ),
      QueueSourceType.playlist => bridge.listPlaylistTracks(
        playlistId: source.playlistId ?? 0,
        query: '',
      ),
      _ => bridge.listTracks(folder: '', recursive: true, query: ''),
    };
  }

  static List<QueueItem> buildLocalQueueItems(Iterable<TrackLite> tracks) {
    return tracks.map(_localQueueItemFromTrackLite).toList();
  }

  static int findTrackIdIndex({
    required List<QueueItem> items,
    required BigInt trackId,
  }) {
    for (var i = 0; i < items.length; i++) {
      if (items[i].trackId == trackId) {
        return i;
      }
    }
    return -1;
  }

  static QueueItem _localQueueItemFromTrackLite(TrackLite track) {
    return QueueItem(
      trackId: null,
      path: track.path,
      id: track.id.toInt() >= 0 ? track.id.toInt() : null,
      title: track.title,
      artist: track.artist,
      album: track.album,
      durationMs: track.durationMs?.toInt(),
    );
  }
}
