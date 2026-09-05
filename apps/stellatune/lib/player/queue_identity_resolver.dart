import 'queue_models.dart';

/// Resolves local identities in one bridge call without losing duplicate entries.
/// Provider registration is sequential so a queue cannot flood the database pool.
Future<List<BigInt>> resolveQueueTrackIds(
  List<QueueItem> items, {
  required Future<List<BigInt>> Function(List<int>) ensureLocalTracks,
  required Future<BigInt> Function(ProviderQueueTrack) ensureProviderTrack,
}) async {
  final localIds = <int>{};
  for (final item in items) {
    if (item.trackId != null || item.providerTrack != null) continue;
    final id = item.id;
    if (id == null || id <= 0) {
      throw StateError('Local queue item has no Library TrackId');
    }
    localIds.add(id);
  }
  final localList = localIds.toList();
  final resolved = localList.isEmpty
      ? <BigInt>[]
      : await ensureLocalTracks(localList);
  if (resolved.length != localList.length) {
    throw StateError('Local track registration returned an incomplete result');
  }
  final localTracks = {
    for (var i = 0; i < localList.length; i++) localList[i]: resolved[i],
  };
  final result = <BigInt>[];
  for (final item in items) {
    result.add(
      item.trackId ??
          (item.providerTrack == null
              ? localTracks[item.id]!
              : await ensureProviderTrack(item.providerTrack!)),
    );
  }
  return result;
}
