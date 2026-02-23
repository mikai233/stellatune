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

  static int findTrackRefIndex({
    required List<QueueItem> items,
    required TrackRef track,
  }) {
    final resumeKey = stableTrackKey(track);
    for (var i = 0; i < items.length; i++) {
      if (items[i].stableTrackKey == resumeKey) {
        return i;
      }
    }
    return -1;
  }

  static String stableTrackKey(TrackRef track) =>
      '${track.sourceId}:${track.trackId}';

  static TrackRef localTrackRef(String path) =>
      TrackRef(sourceId: 'local', trackId: path, locator: path);

  static QueueItem _localQueueItemFromTrackLite(TrackLite track) {
    return QueueItem(
      track: localTrackRef(track.path),
      id: track.id.toInt() >= 0 ? track.id.toInt() : null,
      title: track.title,
      artist: track.artist,
      album: track.album,
      durationMs: track.durationMs?.toInt(),
    );
  }
}
