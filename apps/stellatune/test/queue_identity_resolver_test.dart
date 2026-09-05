import 'package:flutter_test/flutter_test.dart';
import 'package:stellatune/player/queue_identity_resolver.dart';
import 'package:stellatune/player/queue_models.dart';

void main() {
  test(
    'a library-sized queue uses one registration and preserves duplicates',
    () async {
      final items = [
        for (var id = 1; id <= 10000; id++)
          QueueItem(trackId: null, path: '$id', id: id),
        const QueueItem(trackId: null, path: '1', id: 1),
        QueueItem(trackId: BigInt.from(99), path: 'known'),
      ];
      var calls = 0;
      final result = await resolveQueueTrackIds(
        items,
        ensureLocalTracks: (ids) async {
          calls++;
          expect(ids.length, 10000);
          return ids.map((id) => BigInt.from(id + 100)).toList();
        },
        ensureProviderTrack: (_) async =>
            throw StateError('unexpected provider'),
      );
      expect(calls, 1);
      expect(result.length, items.length);
      expect(result[0], result[10000]);
      expect(result[9999], BigInt.from(10100));
      expect(result.last, BigInt.from(99));
    },
  );

  test(
    'provider registration does not create unbounded concurrent writes',
    () async {
      var active = 0;
      var maximum = 0;
      final result = await resolveQueueTrackIds(
        [
          for (var id = 1; id <= 20; id++)
            QueueItem(
              trackId: null,
              path: '',
              providerTrack: ProviderQueueTrack(
                providerId: 'provider',
                pluginId: 'plugin',
                typeId: 'source',
                configJson: '{}',
                providerKey: '$id',
              ),
            ),
        ],
        ensureLocalTracks: (_) async =>
            throw StateError('unexpected local batch'),
        ensureProviderTrack: (provider) async {
          active++;
          if (active > maximum) maximum = active;
          await Future<void>.delayed(Duration.zero);
          active--;
          return BigInt.parse(provider.providerKey);
        },
      );
      expect(maximum, 1);
      expect(result, List.generate(20, (i) => BigInt.from(i + 1)));
    },
  );

  test('invalid local identities fail before any registration', () async {
    await expectLater(
      resolveQueueTrackIds(
        const [
          QueueItem(trackId: null, path: 'valid', id: 1),
          QueueItem(trackId: null, path: 'invalid', id: -1),
        ],
        ensureLocalTracks: (_) async => throw Exception('must not register'),
        ensureProviderTrack: (_) async => throw Exception('must not register'),
      ),
      throwsStateError,
    );
  });
}
