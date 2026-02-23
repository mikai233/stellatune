import 'package:stellatune/bridge/bridge.dart';
import 'package:stellatune/player/decoder_extension_support.dart';

typedef PlayabilityReasonLocalizer = String Function(String rawReason);

String trackPlayabilityCacheKey(TrackLite track) => '${track.id}|${track.path}';

String buildTrackResultsKey(List<TrackLite> items) {
  if (items.isEmpty) return '';
  final buffer = StringBuffer();
  for (final track in items) {
    buffer
      ..write(track.id)
      ..write('|')
      ..write(track.path)
      ..write(';');
  }
  return buffer.toString();
}

Map<int, String> buildBlockedReasonByTrackId({
  required List<TrackLite> items,
  required Map<String, String?> playabilityCache,
  required PlayabilityReasonLocalizer localizeReason,
}) {
  final blocked = <int, String>{};
  for (final track in items) {
    final reason = playabilityCache[trackPlayabilityCacheKey(track)];
    if (reason == null) continue;
    blocked[track.id.toInt()] = localizeReason(reason);
  }
  return blocked;
}

bool hasSameTrackBlockedReasons(
  Map<int, String> current,
  Map<int, String> next,
) {
  if (identical(current, next)) return true;
  if (current.length != next.length) return false;
  for (final entry in current.entries) {
    if (next[entry.key] != entry.value) {
      return false;
    }
  }
  return true;
}

class TrackPlayabilityProbe {
  TrackPlayabilityProbe({this.probeMargin = 40, this.cacheMaxEntries = 12000});

  final int probeMargin;
  final int cacheMaxEntries;

  int _requestSeq = 0;
  int _viewportStart = 0;
  int _viewportEnd = -1;
  String _resultsKey = '';
  final Map<String, String?> _playabilityCache = <String, String?>{};

  bool updateViewportRange(int startIndex, int endIndex) {
    if (_viewportStart == startIndex && _viewportEnd == endIndex) {
      return false;
    }
    _viewportStart = startIndex;
    _viewportEnd = endIndex;
    return true;
  }

  Map<int, String> buildBlockedReasons(
    List<TrackLite> items, {
    required PlayabilityReasonLocalizer localizeReason,
  }) {
    return buildBlockedReasonByTrackId(
      items: items,
      playabilityCache: _playabilityCache,
      localizeReason: localizeReason,
    );
  }

  Future<Map<int, String>?> refreshBlockedReasons({
    required List<TrackLite> items,
    required PlayabilityReasonLocalizer localizeReason,
    required Future<void> Function() ensureDecoderSupport,
    required DecoderExtensionSupportSnapshot? Function() readDecoderSnapshot,
    bool force = false,
  }) async {
    final key = buildTrackResultsKey(items);
    if (_resultsKey != key) {
      _resultsKey = key;
      _viewportStart = 0;
      _viewportEnd = -1;
    }

    if (items.isEmpty) {
      return const <int, String>{};
    }

    final currentBlocked = buildBlockedReasons(
      items,
      localizeReason: localizeReason,
    );

    final maxIndex = items.length - 1;
    final initialEnd = (items.length - 1).clamp(0, 19).toInt();
    var probeStart = _viewportEnd >= 0 ? _viewportStart : 0;
    var probeEnd = _viewportEnd >= 0 ? _viewportEnd : initialEnd;
    probeStart = (probeStart - probeMargin).clamp(0, maxIndex).toInt();
    probeEnd = (probeEnd + probeMargin).clamp(0, maxIndex).toInt();
    if (probeEnd < probeStart) {
      probeEnd = probeStart;
    }

    final pending = <(String, String)>[];
    for (var i = probeStart; i <= probeEnd; i++) {
      final track = items[i];
      final cacheKey = trackPlayabilityCacheKey(track);
      if (!force && _playabilityCache.containsKey(cacheKey)) {
        continue;
      }
      pending.add((cacheKey, track.path));
    }
    if (pending.isEmpty) {
      return currentBlocked;
    }

    final requestSeq = ++_requestSeq;
    await ensureDecoderSupport();
    final snapshot = readDecoderSnapshot();
    if (snapshot == null || requestSeq != _requestSeq) {
      return null;
    }

    for (final item in pending) {
      _playabilityCache[item.$1] = snapshot.canPlayLocalPath(item.$2)
          ? null
          : 'no_decoder_for_local_track';
    }
    _evictPlayabilityCacheIfNeeded();

    return buildBlockedReasons(items, localizeReason: localizeReason);
  }

  void _evictPlayabilityCacheIfNeeded() {
    while (_playabilityCache.length > cacheMaxEntries) {
      if (_playabilityCache.isEmpty) return;
      _playabilityCache.remove(_playabilityCache.keys.first);
    }
  }
}
